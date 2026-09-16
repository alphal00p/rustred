//! Prepared native coefficient charts for dynamic original-domain replay.
//!
//! The defining equations and fixed face are reduced again using the same
//! Symbolica routine as the search's `AffineCase`. Persisted matrices and chart
//! flags are compared with that reconstruction, never used as authority.

use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::{IntegerRing, Matrix};

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::foundry::completion::LatticeBox;
use crate::solver::{AffineGeometryError, AffineRestrictionChart, canonical_equalities};

use super::AffineApplicationDomain;

/// One chart prepared once per original-source parent, not once per term.
/// This restricts coefficient values without changing integral columns or
/// claiming feasibility of integer assignments to rational free coordinates.
#[derive(Clone, Debug)]
pub(crate) struct AffineDomainRestriction {
    template: CoefficientPolynomial,
    fixed: Box<[Option<i16>]>,
    indices: Box<[usize]>,
    chart: AffineRestrictionChart,
}

impl AffineApplicationDomain {
    /// Bool-only reuse of the existing original-equation/box contradiction
    /// service. This transient carrier has no authenticated matrix or chart
    /// and cannot escape as publication evidence. Caller owns input/budget
    /// admission; unsupported geometry is inconclusive, never a proof.
    pub(crate) fn equation_proved_empty_in_box(
        equation: &CoefficientPolynomial,
        indices: &[usize],
        sector: &[bool],
        cell: &LatticeBox,
    ) -> Result<bool, AffineGeometryError> {
        catch_unwind(AssertUnwindSafe(|| {
            let transient = Self {
                sector: sector.to_vec().into_boxed_slice(),
                fixed: vec![None; sector.len()].into_boxed_slice(),
                indices: indices.to_vec().into_boxed_slice(),
                equations: vec![equation.clone()].into_boxed_slice(),
                primitive_matrix: None,
                integral_chart: None,
            };
            transient.is_proved_empty_in_box(cell)
        }))
        .map_err(|_| AffineGeometryError::NativeAlgebra)
    }

    pub(crate) fn prepare_restriction(
        &self,
    ) -> Result<AffineDomainRestriction, AffineGeometryError> {
        catch_unwind(AssertUnwindSafe(|| self.prepare_restriction_native()))
            .map_err(|_| AffineGeometryError::NativeAlgebra)?
    }

    fn prepare_restriction_native(&self) -> Result<AffineDomainRestriction, AffineGeometryError> {
        let invalid = AffineGeometryError::InvalidInput;
        let n = self.sector.len();
        if n == 0 || self.fixed.len() != n || self.indices.len() != n {
            return Err(invalid("affine replay domain arity"));
        }
        if self
            .fixed
            .iter()
            .zip(&self.sector)
            .any(|(fixed, &active)| fixed.is_some_and(|value| (value > 0) != active))
        {
            return Err(invalid("affine replay fixed face outside sector"));
        }
        let Some(template) = self.equations.first() else {
            return Err(invalid("affine replay domain has no equations"));
        };
        let Some((matrix, primitive)) =
            canonical_equalities(&self.fixed, &self.equations, &self.indices)?
        else {
            return Err(invalid(
                "affine replay domain is an inconsistent equality system",
            ));
        };
        // The search moves every equality fixing one coordinate into its
        // coordinate face. Verify this invariant before reusing the shared
        // Chart (whose replacements deliberately omit those fixed rows).
        let mut derived_fixed = vec![None; n];
        let mut coupled_rows = 0;
        for row in matrix.row_iter() {
            let mut nonzero = row[..n]
                .iter()
                .enumerate()
                .filter(|(_, value)| !value.is_zero());
            let Some((pivot, _)) = nonzero.next() else {
                return Err(invalid("affine replay canonical matrix has an empty row"));
            };
            if nonzero.next().is_some() {
                coupled_rows += 1;
            } else {
                if !row[n].is_integer() {
                    return Err(invalid("affine replay fixed coordinate is not integral"));
                }
                derived_fixed[pivot] = Some(
                    row[n]
                        .numerator()
                        .to_i64()
                        .and_then(|value| i16::try_from(value).ok())
                        .ok_or(invalid("affine replay fixed coordinate is out of range"))?,
                );
            }
        }
        if derived_fixed.as_slice() != self.fixed.as_ref() || coupled_rows == 0 {
            return Err(invalid(
                "affine replay fixed face is not canonical for its equations",
            ));
        }
        let chart = AffineRestrictionChart::new(template, &matrix, &self.indices);
        if self.primitive_matrix.as_ref() != Some(&primitive)
            || self.integral_chart != Some(chart.is_integral())
        {
            return Err(invalid(
                "affine replay cached matrix/chart differs from defining equations",
            ));
        }
        Ok(AffineDomainRestriction {
            template: template.zero(),
            fixed: self.fixed.clone(),
            indices: self.indices.clone(),
            chart,
        })
    }
}

impl AffineDomainRestriction {
    /// Transient chart of an admitted affine conjunction, not a persisted
    /// application domain. The caller owns resource preflight and all sector,
    /// context and domain obligations. `None` is a native exact contradiction;
    /// an unrepresentable fixed row is an error, never an empty locus.
    pub(crate) fn from_equalities(
        fixed: &[Option<i16>],
        equations: &[CoefficientPolynomial],
        indices: &[usize],
    ) -> Result<Option<(Self, Matrix<IntegerRing>)>, AffineGeometryError> {
        catch_unwind(AssertUnwindSafe(|| {
            let Some((matrix, primitive)) = canonical_equalities(fixed, equations, indices)? else {
                return Ok(None);
            };
            let n = indices.len();
            let mut derived_fixed = vec![None; n];
            for row in matrix.row_iter() {
                let mut nonzero = row[..n]
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| !value.is_zero());
                let Some((axis, _)) = nonzero.next() else {
                    return Err(AffineGeometryError::NativeAlgebra);
                };
                if nonzero.next().is_none() {
                    if !row[n].is_integer() {
                        return Err(AffineGeometryError::NativeAlgebra);
                    }
                    derived_fixed[axis] = Some(
                        row[n]
                            .numerator()
                            .to_i64()
                            .and_then(|value| i16::try_from(value).ok())
                            .ok_or(AffineGeometryError::InvalidInput(
                                "transient affine fixed coordinate is out of range",
                            ))?,
                    );
                }
            }
            // Chart replacements deliberately omit coordinate-only rows.
            // Keep every newly derived fixed value alongside the coupled pivots.
            let chart = AffineRestrictionChart::new(&equations[0], &matrix, indices);
            Ok(Some((
                Self {
                    template: equations[0].zero(),
                    fixed: derived_fixed.into_boxed_slice(),
                    indices: indices.to_vec().into_boxed_slice(),
                    chart,
                },
                primitive,
            )))
        }))
        .map_err(|_| AffineGeometryError::NativeAlgebra)?
    }

    /// Whether an authenticated input contains any fixed or pivot variable
    /// replaced by this prepared chart. `false` proves that substitution is
    /// unnecessary, not that the polynomial is nonzero. Zero-locus restriction
    /// may still discard nonzero scalar content; coefficient-value APIs must
    /// continue preserving their exact relative normalization.
    pub(crate) fn affects_polynomial(
        &self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<bool, AffineGeometryError> {
        self.validate_polynomial(polynomial)?;
        Ok(self
            .fixed
            .iter()
            .zip(&self.indices)
            .any(|(fixed, &position)| fixed.is_some() && polynomial.contains(position))
            || self.chart.replaces_variable_in(polynomial))
    }

    /// Bound native substitution's intermediate term count and the largest
    /// individual monomial expansion, using only authenticated support. Fixed
    /// scalar substitutions run first; a fixed zero kills its entire monomial.
    /// Coupled pivot replacements contain no other pivots, so each surviving
    /// monomial expands by at most the product of the actual replacement term
    /// counts raised to that variable's exponent. No cancellation is assumed.
    pub(crate) fn restriction_term_bound(
        &self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<(usize, usize), AffineGeometryError> {
        self.validate_polynomial(polynomial)?;
        let overflow =
            || AffineGeometryError::InvalidInput("affine restriction term bound overflow");
        let mut terms = 0usize;
        let mut expansion = 1usize;
        for powers in polynomial.exponents_iter() {
            if self
                .fixed
                .iter()
                .zip(&self.indices)
                .any(|(fixed, &position)| *fixed == Some(0) && powers[position] != 0)
            {
                continue;
            }
            let mut monomial_terms = 1usize;
            let mut include = |position: usize, replacement_terms: usize| {
                let factor = replacement_terms
                    .max(1)
                    .checked_pow(u32::from(powers[position]))
                    .ok_or_else(overflow)?;
                monomial_terms = monomial_terms.checked_mul(factor).ok_or_else(overflow)?;
                Ok::<(), AffineGeometryError>(())
            };
            match &self.chart {
                AffineRestrictionChart::Integral(replacements) => {
                    for (position, replacement) in replacements {
                        include(*position, replacement.nterms())?;
                    }
                }
                AffineRestrictionChart::Rational(replacements) => {
                    for (position, replacement) in replacements {
                        include(*position, replacement.nterms())?;
                    }
                }
            }
            expansion = expansion.max(monomial_terms);
            terms = terms.checked_add(monomial_terms).ok_or_else(overflow)?;
        }
        // The native implementation first retains/specializes the full input.
        // Keep that allocation covered even if a fixed zero removes terms.
        Ok((terms.max(polynomial.nterms()), expansion))
    }

    /// Restrict numerator and denominator jointly. Relative rational scale is
    /// retained and an identically zero restricted denominator is an error.
    pub(crate) fn restrict_coefficient(
        &self,
        coefficient: &Coefficient,
    ) -> Result<Coefficient, AffineGeometryError> {
        self.validate_polynomial(&coefficient.numerator)?;
        self.validate_polynomial(&coefficient.denominator)?;
        self.chart
            .restrict_coefficient_on_face(coefficient, &self.fixed, &self.indices)
    }

    /// Exact polynomial *value*, including rational factors introduced by a
    /// non-integral chart. Use this, not `restrict_equation`, for coefficients.
    pub(crate) fn restrict_polynomial_value(
        &self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<Coefficient, AffineGeometryError> {
        self.validate_polynomial(polynomial)?;
        Ok(self
            .chart
            .restrict_polynomial_value_on_face(polynomial, &self.fixed, &self.indices))
    }

    /// Restrict a zero-locus equation; only this method may discard nonzero
    /// scalar content when clearing rational chart denominators.
    pub(crate) fn restrict_equation(
        &self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<CoefficientPolynomial, AffineGeometryError> {
        self.validate_polynomial(polynomial)?;
        Ok(self
            .chart
            .restrict_equation_on_face(polynomial, &self.fixed, &self.indices))
    }

    fn validate_polynomial(
        &self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<(), AffineGeometryError> {
        if polynomial.variables() != self.template.variables()
            || polynomial
                .coefficients
                .len()
                .checked_mul(polynomial.nvars())
                != Some(polynomial.exponents.len())
        {
            return Err(AffineGeometryError::InvalidInput(
                "polynomial and affine replay chart use different maps or malformed storage",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "restriction/support_tests.rs"]
mod support_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};
    use symbolica::prelude::{Integer, Matrix, Z};

    fn domain<const N: usize>(
        context: &CoefficientContext,
        fixed: [Option<i16>; N],
        indices: [usize; N],
        equations: &[&str],
    ) -> AffineApplicationDomain {
        let equations: Vec<_> = equations
            .iter()
            .map(|equation| context.coefficient_fixture(equation).numerator)
            .collect();
        let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
            &CoordinateCase::new(fixed).unwrap(),
            &equations,
            &indices,
            &[true; N],
        )
        .unwrap() else {
            panic!("test must have an affine chart")
        };
        AffineApplicationDomain::from_case(&case, &[true; N]).unwrap()
    }

    #[test]
    fn rational_chart_preserves_exact_values_and_denominator_obligations() {
        let context = CoefficientContext::new(["d", "n0", "n1"]);
        let prepared = domain(&context, [None; 2], [1, 2], &["2*n0-n1"])
            .prepare_restriction()
            .unwrap();
        assert_eq!(
            prepared
                .restrict_polynomial_value(&context.coefficient_fixture("n0").numerator,)
                .unwrap(),
            context.coefficient_fixture("n1/2")
        );
        assert_eq!(
            prepared
                .restrict_coefficient(&context.coefficient_fixture("d*n0/(n0+1)"),)
                .unwrap(),
            context.coefficient_fixture("d*n1/(n1+2)")
        );
        assert_eq!(
            prepared.restrict_coefficient(&context.coefficient_fixture("1/(2*n0-n1)"),),
            Err(AffineGeometryError::UndefinedCoefficient)
        );
        assert!(
            prepared
                .restrict_equation(&context.coefficient_fixture("2*n0-n1").numerator,)
                .unwrap()
                .is_zero()
        );
    }

    #[test]
    fn translated_coefficients_are_restricted_after_translation() {
        let context = CoefficientContext::new(["n0", "n1"]);
        let prepared = domain(&context, [None; 2], [0, 1], &["2*n0-n1"])
            .prepare_restriction()
            .unwrap();
        let original = context.coefficient_fixture("2*n0-n1").numerator;
        assert!(
            prepared
                .restrict_polynomial_value(&original)
                .unwrap()
                .is_zero()
        );
        assert_eq!(
            prepared
                .restrict_polynomial_value(&original.shift_var(0, &Integer::one()),)
                .unwrap(),
            context.integer(2)
        );
        assert_eq!(
            prepared
                .restrict_coefficient(&context.coefficient_fixture("(n0+1)/(n1+2)"),)
                .unwrap(),
            context.coefficient_fixture("1/2")
        );
    }

    #[test]
    fn fixed_face_and_multiple_rational_pivots_share_the_search_chart() {
        let context = CoefficientContext::new(["d", "n0", "n1", "n2", "n3"]);
        let equations = ["2*n0-n2-4", "3*n1-n2-6"];
        let prepared = domain(
            &context,
            [None, None, None, Some(3)],
            [1, 2, 3, 4],
            &equations,
        )
        .prepare_restriction()
        .unwrap();
        assert_eq!(
            prepared
                .restrict_polynomial_value(&context.coefficient_fixture("6*n0*n1+n3").numerator,)
                .unwrap(),
            context.coefficient_fixture("(n2+4)*(n2+6)+3")
        );
    }

    #[test]
    fn foreign_maps_and_mutated_cached_witnesses_are_rejected() {
        let context = CoefficientContext::new(["d", "n0", "n1"]);
        let mut affine = domain(&context, [None; 2], [1, 2], &["2*n0-n1"]);
        let prepared = affine.prepare_restriction().unwrap();
        let foreign = CoefficientContext::new(["n0", "d", "n1"]);
        assert!(
            prepared
                .restrict_coefficient(&foreign.coefficient_fixture("n0"))
                .is_err()
        );
        assert!(
            prepared
                .restrict_equation(&foreign.coefficient_fixture("n0").numerator)
                .is_err()
        );
        affine.primitive_matrix = Some(
            Matrix::from_linear(
                vec![Integer::from(1), Integer::from(-1), Integer::zero()],
                1,
                3,
                Z,
            )
            .unwrap(),
        );
        assert!(affine.prepare_restriction().is_err());
        affine = domain(&context, [None; 2], [1, 2], &["2*n0-n1"]);
        affine.integral_chart = Some(true);
        assert!(affine.prepare_restriction().is_err());
        affine.indices = vec![1, 1].into();
        assert!(affine.prepare_restriction().is_err());
    }

    #[test]
    fn nonlinear_parameter_and_noncanonical_fixed_definitions_fail_closed() {
        let context = CoefficientContext::new(["d", "n0", "n1"]);
        for equation in ["n0*n1", "n0+d", "n0-1", "1"] {
            let mut affine = domain(&context, [None; 2], [1, 2], &["2*n0-n1"]);
            affine.equations = vec![context.coefficient_fixture(equation).numerator].into();
            assert!(affine.prepare_restriction().is_err(), "accepted {equation}");
        }
    }
}
