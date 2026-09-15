//! Exact affine application-domain evidence retained at the source-port gate.
//!
//! The artifact installer still accepts rectangular `LatticeBox` domains only.
//! This carrier deliberately stops one boundary earlier: it preserves the
//! canonical affine face and its Symbolica-owned equality polynomials so that
//! a future affine owner can consume the evidence without rediscovering it.
//! It does not provide an integer-polyhedron solver, a rectangular hull, or a
//! sampled approximation.  Callers which cannot prove affine coverage must
//! continue to fail closed.

use std::fmt;

use crate::algebra::CoefficientPolynomial;
use crate::solver::AffineCase;

/// Topology-neutral exact affine equality domain in original index variables.
///
/// `sector` and `fixed` describe the surrounding sign orthant and coordinate
/// face. `equations` are the coupled equalities, retained exactly as admitted
/// by [`AffineCase`] (and therefore with Symbolica's canonical variable map).
/// No claim is made that the corresponding integer/sector locus is non-empty
/// or that a finite box cover exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AffineApplicationDomain {
    sector: Box<[bool]>,
    fixed: Box<[Option<i16>]>,
    equations: Box<[CoefficientPolynomial]>,
}

impl AffineApplicationDomain {
    /// Capture an admitted affine case without changing or re-computing its
    /// Symbolica representation.
    pub(crate) fn from_case<const N: usize>(
        case: &AffineCase<N>,
        sector: &[bool; N],
    ) -> Result<Self, AffineApplicationDomainError> {
        if !case.face().is_in_sector(sector) {
            return Err(AffineApplicationDomainError::OutsideSector);
        }
        if case.equations().is_empty() {
            return Err(AffineApplicationDomainError::MissingCoupledEquation);
        }
        Ok(Self {
            sector: sector.to_vec().into_boxed_slice(),
            fixed: case.face().fixed().to_vec().into_boxed_slice(),
            equations: case.equations().to_vec().into_boxed_slice(),
        })
    }

    /// Construct a carrier from already authenticated pieces at a replay
    /// boundary. This performs only shape checks; polynomial admission remains
    /// the responsibility of the `AffineCase`/source solver boundary.
    pub(crate) fn try_new(
        sector: impl Into<Box<[bool]>>,
        fixed: impl Into<Box<[Option<i16>]>>,
        equations: impl Into<Box<[CoefficientPolynomial]>>,
    ) -> Result<Self, AffineApplicationDomainError> {
        let sector = sector.into();
        let fixed = fixed.into();
        let equations = equations.into();
        if sector.is_empty() || sector.len() != fixed.len() {
            return Err(AffineApplicationDomainError::ArityMismatch);
        }
        if equations.is_empty() {
            return Err(AffineApplicationDomainError::MissingCoupledEquation);
        }
        let variables = equations[0].variables().clone();
        if equations
            .iter()
            .any(|equation| equation.variables() != &variables)
        {
            return Err(AffineApplicationDomainError::VariableMapMismatch);
        }
        Ok(Self {
            sector,
            fixed,
            equations,
        })
    }

    pub(crate) fn sector(&self) -> &[bool] {
        &self.sector
    }

    pub(crate) fn fixed(&self) -> &[Option<i16>] {
        &self.fixed
    }

    pub(crate) fn equations(&self) -> &[CoefficientPolynomial] {
        &self.equations
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AffineApplicationDomainError {
    ArityMismatch,
    MissingCoupledEquation,
    VariableMapMismatch,
    OutsideSector,
}

impl fmt::Display for AffineApplicationDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArityMismatch => {
                formatter.write_str("affine application domain has incompatible arity")
            }
            Self::MissingCoupledEquation => {
                formatter.write_str("affine application domain has no coupled equation")
            }
            Self::VariableMapMismatch => formatter
                .write_str("affine application domain equations use different variable maps"),
            Self::OutsideSector => {
                formatter.write_str("affine application domain lies outside its sector")
            }
        }
    }
}

impl std::error::Error for AffineApplicationDomainError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;

    #[test]
    fn captures_exact_case_without_rectangularizing_it() {
        let context = CoefficientContext::new(["n0", "n1"]);
        let equation = context.coefficient_fixture("n0 - n1").numerator;
        let case = AffineCase::from_coordinate(
            &crate::solver::CoordinateCase::generic(),
            &[equation.clone()],
            &[0, 1],
            &[true, true],
        )
        .unwrap();
        let crate::solver::AffineIntersection::Affine(case) = case else {
            panic!("fixture must remain coupled");
        };
        let domain = AffineApplicationDomain::from_case(&case, &[true, true]).unwrap();
        assert_eq!(domain.sector(), &[true, true]);
        assert_eq!(domain.fixed(), &[None, None]);
        assert_eq!(domain.equations(), &[equation]);
    }

    #[test]
    fn rejects_a_case_outside_the_declared_sector() {
        let context = CoefficientContext::new(["n0", "n1"]);
        let equation = context.coefficient_fixture("n0 - n1").numerator;
        let crate::solver::AffineIntersection::Affine(case) = AffineCase::from_coordinate(
            &crate::solver::CoordinateCase::generic(),
            &[equation],
            &[0, 1],
            &[true, true],
        )
        .unwrap() else {
            panic!("fixture must remain coupled");
        };
        assert_eq!(
            AffineApplicationDomain::from_case(&case, &[true, false]),
            Err(AffineApplicationDomainError::OutsideSector)
        );
    }
}
