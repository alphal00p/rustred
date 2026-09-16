use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::algebra::CoefficientPolynomial;

use super::super::{AffineGeometryError, Case};

/// Cold exceptional-geometry work limits. Native calls are checked before and
/// after execution; these limits do not interrupt a running native algorithm or
/// promise a hard bound on its temporary memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaseIntersectionLimits {
    pub max_work_items: usize,
    pub max_terms_per_conjunction: usize,
    pub max_normalizations: usize,
    pub max_factorizations: usize,
}

impl Default for CaseIntersectionLimits {
    fn default() -> Self {
        Self {
            max_work_items: 4096,
            max_terms_per_conjunction: 100_000,
            max_normalizations: 1024,
            max_factorizations: 4096,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseIntersectionBudget {
    WorkItems,
    ConjunctionTerms,
    Normalizations,
    Factorizations,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaseIntersectionStats {
    pub work_items: usize,
    pub restrictions: usize,
    pub affine_admissions: usize,
    pub normalizations: usize,
    pub factorizations: usize,
    pub factor_children: usize,
    pub peak_conjunction_terms: usize,
    pub restriction_time: Duration,
    pub admission_time: Duration,
    pub normalization_time: Duration,
    pub factorization_time: Duration,
    pub union_time: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseIntersectionResult<const N: usize> {
    /// Exact OR of admitted cases. An empty vector means proved empty.
    pub cases: Vec<Case<N>>,
    pub stats: CaseIntersectionStats,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaseIntersectionFailure {
    InvalidInput(&'static str),
    Admission(AffineGeometryError),
    NativeAlgebra,
    /// Exact operations made no progress towards coordinate/affine admission.
    UnsupportedGeometry,
    /// A canonical work state repeated. It is not treated as an empty branch.
    RepeatedState,
    Budget {
        kind: CaseIntersectionBudget,
        limit: usize,
    },
}

/// Atomic failure of the complete OR result, including original provenance and
/// the unresolved AND branch. Resolved siblings are deliberately not returned
/// as a successful cover. Coefficient polynomials are equations equal to zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseIntersectionError<const N: usize> {
    pub original_parent: Case<N>,
    pub original_conjunction: Arc<[CoefficientPolynomial]>,
    pub unresolved_parent: Case<N>,
    pub unresolved_conjunction: Arc<[CoefficientPolynomial]>,
    pub failure: CaseIntersectionFailure,
    pub stats: CaseIntersectionStats,
}

impl<const N: usize> fmt::Display for CaseIntersectionError<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "incomplete exact case intersection: ")?;
        match &self.failure {
            CaseIntersectionFailure::InvalidInput(message) => f.write_str(message),
            CaseIntersectionFailure::Admission(error) => error.fmt(f),
            CaseIntersectionFailure::NativeAlgebra => f.write_str("native algebra failed"),
            CaseIntersectionFailure::UnsupportedGeometry => {
                f.write_str("a branch retains unsupported nonlinear equalities")
            }
            CaseIntersectionFailure::RepeatedState => f.write_str("a work state repeated"),
            CaseIntersectionFailure::Budget { kind, limit } => {
                write!(f, "{kind:?} budget {limit} exhausted")
            }
        }?;
        super::diagnostic::write_context(self, f)
    }
}

impl<const N: usize> std::error::Error for CaseIntersectionError<N> {}
