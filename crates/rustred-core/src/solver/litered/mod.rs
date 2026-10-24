//! UNCERTIFIED LiteRed lane (RR-8): a modular application DAG over saved
//! `family-candidates` formulas, residual compression with the RustRed
//! sector-symmetry census, and coefficients onto the compressed basis.
//!
//! What this lane is:
//! - the exact rule dispatch of the candidate applier (`CandidateEvaluator`
//!   semantics: ordered `Partition` rules, whole-piece alternatives, fixed
//!   coordinates, affine equalities, AND-branch exclusions, every RHS
//!   denominator, zero-sector children, strict descent), evaluated through
//!   Zp images whose zeros are always confirmed by exact specialization, so
//!   the DAG structure is exact and independent of the prime and point;
//! - forward (memoized) and adjoint sweeps of that DAG mod p;
//! - compression of the candidate terminals with exact census identities
//!   `I - T(I) = 0`, pivoting on the hardest column, so the surviving
//!   columns (the basis) sit at the easy end of a declared column order;
//! - exact rational coefficients at rational parameter points by multi-prime
//!   CRT and maximal-quotient rational reconstruction, verified at a fresh
//!   prime.
//!
//! What it is not: candidate formulas carry no source-replay provenance or
//! cover proof, and the compressed basis is not proved minimal. Use it as a
//! cross-check and as an interim backend; never feed it to closing-artifact
//! installation or publication. Families with masses other than one, extra
//! parameters (`eta`) or a numeric dimension enter through
//! [`crate::solver::CandidateFamilyAdmission::ParametricVacuum`].

mod compress;
mod dag;
mod dynamic;
mod generate;
mod modular;
mod reconstruct;
mod session;
mod zp;

#[cfg(test)]
mod tests;

pub use compress::{
    CensusRow, CensusRowStatistics, ColumnOrder, ColumnSpace, Elimination, RowOrigin, SymmetryRows,
    census_rows, compare_hardness,
};
pub use dag::{DagStatistics, DagStructure, DagValues, NodeKind, SampleProgramStatistics};
pub use dynamic::{DynamicLiteRedSession, LiteRedLaneReport};
pub use generate::{
    CandidateGenerationStatistics, generate_candidate_program, isps_at_ends_priority,
};
pub use modular::SparseVector;
pub use reconstruct::RationalFunction;
pub use session::{
    ClassCount, CompressionReport, ExactPointReduction, LiteRedOptions, LiteRedSession,
    SampleReduction, UnivariateReduction,
};
pub use zp::{
    DEFAULT_PRIME, PrimeField, ZpIndexPolynomial, ZpSample, is_prime_u64, reconstruction_primes,
};

use crate::algebra::IndexedAlgebraError;
use crate::solver::CandidateReductionError;

/// Errors of the LiteRed lane. `UnluckySample` means a Zp image vanished
/// where the exact value does not; the caller resamples (other prime/point).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiteRedError {
    InvalidInput(String),
    Candidate(CandidateReductionError),
    Algebra(String),
    Census(String),
    UnluckySample {
        phase: &'static str,
        detail: String,
    },
    UncoveredNode {
        node: u32,
    },
    Limit {
        resource: &'static str,
        limit: usize,
    },
    Unstable(String),
}

impl LiteRedError {
    pub(crate) fn from_indexed(error: IndexedAlgebraError) -> Self {
        Self::Candidate(CandidateReductionError::from(error))
    }
}

impl From<CandidateReductionError> for LiteRedError {
    fn from(error: CandidateReductionError) -> Self {
        Self::Candidate(error)
    }
}

impl std::fmt::Display for LiteRedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(detail) => write!(f, "invalid LiteRed input: {detail}"),
            Self::Candidate(error) => write!(f, "candidate program: {error}"),
            Self::Algebra(detail) => write!(f, "exact algebra: {detail}"),
            Self::Census(detail) => write!(f, "sector-symmetry census: {detail}"),
            Self::UnluckySample { phase, detail } => {
                write!(f, "unlucky modular sample ({phase}): {detail}")
            }
            Self::UncoveredNode { node } => {
                write!(f, "DAG node {node} has no applicable candidate rule")
            }
            Self::Limit { resource, limit } => write!(f, "{resource} exceeds {limit}"),
            Self::Unstable(detail) => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for LiteRedError {}
