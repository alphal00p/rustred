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

use symbolica::prelude::{Integer, IntegerRing, Matrix};

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
pub struct AffineApplicationDomain {
    sector: Box<[bool]>,
    fixed: Box<[Option<i16>]>,
    /// Positions of the original integral-index variables in the shared
    /// Symbolica coefficient variable map.  Keeping this map with the
    /// equality carrier is essential: the coefficient context may contain
    /// parameters before the index variables, and a positional assumption
    /// would silently test the wrong locus at runtime.
    indices: Box<[usize]>,
    equations: Box<[CoefficientPolynomial]>,
    /// Primitive augmented rows `[A | b]` copied from the authenticated
    /// `AffineCase`.  Keeping these rows alongside the equations gives a
    /// future coverage compiler an exact integer-lattice witness without
    /// asking it to redo elimination.  `None` is used by the test-only loose
    /// constructor, which intentionally is not admissible as publication
    /// evidence.
    primitive_matrix: Option<Matrix<IntegerRing>>,
    /// Whether the affine chart maps free integer coordinates to integer
    /// dependent coordinates.  This is a chart property, not a proof that
    /// the sector has an integer point or that a family is covered.
    integral_chart: Option<bool>,
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
            indices: case.index_variables().to_vec().into_boxed_slice(),
            equations: case.equations().to_vec().into_boxed_slice(),
            primitive_matrix: Some(case.primitive_matrix().clone()),
            integral_chart: Some(case.has_integral_chart()),
        })
    }

    /// Reconstitute the exact witness carried by a durable source-port plan.
    ///
    /// This is deliberately a structural/authentication boundary, not an
    /// affine solver: the producer has already proved the case and persisted
    /// the canonical sparse equations and primitive matrix.  We nevertheless
    /// reject malformed shapes, duplicate/out-of-range variable positions,
    /// zero rows, and inconsistent variable maps before the witness can reach
    /// replay.  Coverage publication remains fail-closed until its partition
    /// certificate is implemented by the installer.
    pub(crate) fn from_persisted(
        sector: Box<[bool]>,
        fixed: Box<[Option<i16>]>,
        indices: Box<[usize]>,
        equations: Box<[CoefficientPolynomial]>,
        primitive_matrix: Matrix<IntegerRing>,
        integral_chart: bool,
    ) -> Result<Self, AffineApplicationDomainError> {
        if sector.is_empty()
            || fixed.len() != sector.len()
            || indices.len() != sector.len()
            || equations.is_empty()
            || primitive_matrix.ncols() != sector.len() + 1
            || primitive_matrix.nrows() == 0
            || primitive_matrix
                .row_iter()
                .any(|row| row.iter().all(Integer::is_zero))
        {
            return Err(AffineApplicationDomainError::ArityMismatch);
        }
        let variables = equations[0].variables().clone();
        if equations
            .iter()
            .any(|equation| equation.variables() != &variables)
            || indices.iter().any(|&index| index >= variables.len())
        {
            return Err(AffineApplicationDomainError::VariableMapMismatch);
        }
        let mut seen = std::collections::BTreeSet::new();
        if indices.iter().any(|index| !seen.insert(*index)) {
            return Err(AffineApplicationDomainError::VariableMapMismatch);
        }
        Ok(Self {
            sector,
            fixed,
            indices,
            equations,
            primitive_matrix: Some(primitive_matrix),
            integral_chart: Some(integral_chart),
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
            indices: (0..equations[0].nvars())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            equations,
            primitive_matrix: None,
            integral_chart: None,
        })
    }

    pub fn sector(&self) -> &[bool] {
        &self.sector
    }

    pub fn fixed(&self) -> &[Option<i16>] {
        &self.fixed
    }

    /// Original coefficient-variable positions corresponding to the integral
    /// axes.  This is part of the exact domain identity, not an implementation
    /// detail: parameter variables may precede the index variables.
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }

    pub fn equations(&self) -> &[CoefficientPolynomial] {
        &self.equations
    }

    /// Exact primitive integer rows `[A | b]` when this domain came from an
    /// authenticated affine solver case.  The rows are an integer-lattice
    /// witness only; they do not establish feasibility or family coverage.
    pub fn primitive_matrix(&self) -> Option<&Matrix<IntegerRing>> {
        self.primitive_matrix.as_ref()
    }

    /// Whether the solver's canonical chart is integral.  `None` means the
    /// domain was built by the deliberately test-only loose constructor and
    /// therefore cannot be promoted to durable affine coverage evidence.
    pub fn has_integral_chart(&self) -> Option<bool> {
        self.integral_chart
    }

    /// True only for a domain carrying all solver-authenticated integer
    /// evidence.  This is intentionally weaker than a coverage claim: a
    /// caller must still provide and verify a partition certificate for the
    /// surrounding sector before publication.
    pub fn is_authenticated(&self) -> bool {
        self.primitive_matrix.is_some() && self.integral_chart.is_some()
    }

    /// Test an original integral-power assignment against the exact affine
    /// equality locus.  This intentionally does not accept local box
    /// coordinates and never constructs a rectangular hull.  It is a cheap
    /// authenticated predicate for a future affine RuleCell runtime; callers
    /// still have to perform the surrounding sector and guard checks.
    pub(crate) fn contains_powers(&self, powers: &[i64]) -> bool {
        if powers.len() != self.sector.len()
            || self.fixed.len() != powers.len()
            || self.indices.len() != powers.len()
        {
            return false;
        }
        if self
            .sector
            .iter()
            .zip(powers)
            .any(|(&active, &power)| (power >= 1) != active)
        {
            return false;
        }
        if self
            .fixed
            .iter()
            .zip(powers)
            .any(|(fixed, &power)| fixed.is_some_and(|value| i64::from(value) != power))
        {
            return false;
        }
        self.equations.iter().all(|equation| {
            let mut restricted = equation.clone();
            for (&variable, &power) in self.indices.iter().zip(powers) {
                restricted =
                    restricted.replace(variable, &symbolica::prelude::Integer::from(power));
            }
            restricted.is_zero()
        })
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
        assert_eq!(domain.indices(), &[0, 1]);
        assert_eq!(domain.equations(), &[equation]);
        assert!(domain.is_authenticated());
        assert_eq!(domain.has_integral_chart(), Some(true));
        assert!(domain.primitive_matrix().is_some());
        assert!(domain.contains_powers(&[3, 3]));
        assert!(!domain.contains_powers(&[3, 2]));
        assert!(!domain.contains_powers(&[0, 0]));
    }

    #[test]
    fn rejects_a_fixed_face_outside_the_declared_sector() {
        let context = CoefficientContext::new(["n0", "n1", "n2"]);
        let equation = context.coefficient_fixture("n0 - n1").numerator;
        let crate::solver::AffineIntersection::Affine(case) = AffineCase::from_coordinate(
            &crate::solver::CoordinateCase::new([None, None, Some(1)]).unwrap(),
            &[equation],
            &[0, 1, 2],
            &[true, true, true],
        )
        .unwrap() else {
            panic!("fixture must remain coupled");
        };
        assert_eq!(
            AffineApplicationDomain::from_case(&case, &[true, true, false]),
            Err(AffineApplicationDomainError::OutsideSector)
        );
    }

    #[test]
    fn equality_predicate_honours_fixed_face_and_negative_sector() {
        let context = CoefficientContext::new(["n0", "n1", "n2"]);
        let equation = context.coefficient_fixture("n0 + n1 + 1").numerator;
        let crate::solver::AffineIntersection::Affine(case) = AffineCase::from_coordinate(
            &crate::solver::CoordinateCase::new([None, None, Some(-1)]).unwrap(),
            &[equation],
            &[0, 1, 2],
            &[false, false, false],
        )
        .unwrap() else {
            panic!("fixture must remain coupled");
        };
        let domain = AffineApplicationDomain::from_case(&case, &[false, false, false]).unwrap();
        assert!(domain.contains_powers(&[0, -1, -1]));
        assert!(!domain.contains_powers(&[0, -2, -1]));
        assert!(!domain.contains_powers(&[-1, -1, -2]));
    }

    #[test]
    fn loose_constructor_cannot_be_promoted_to_coverage_evidence() {
        let context = CoefficientContext::new(["n0", "n1"]);
        let equation = context.coefficient_fixture("n0 - n1").numerator;
        let domain = AffineApplicationDomain::try_new([true, true], [None, None], [equation])
            .expect("shape-valid diagnostic domain");

        // A shape-valid carrier created outside AffineCase deliberately lacks
        // the canonical integer rows and chart witness.  It may be useful in
        // unit diagnostics, but cannot cross a publication boundary.
        assert!(!domain.is_authenticated());
        assert!(domain.primitive_matrix().is_none());
        assert_eq!(domain.has_integral_chart(), None);
    }
}
