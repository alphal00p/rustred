//! Proof-bearing application cells refined from exact parametric rules.
//!
//! A cell never authors algebra. It retains the generated rule and immutable
//! translated source views, snapshots the original replay proof domain, and
//! separately proves a tightened or exceptional application box. Every
//! retained pivot guard is either proved nonzero over the entire integer box,
//! or (only after regenerated-source replay) retained with an exact separable
//! coordinate zero locus for pointwise routing. The owner-cover compiler keeps
//! those walls geometrically uncovered. Fixed-index specialization uses
//! Symbolica's polynomial substitution; a branch may be pruned only when its
//! coefficient is identically zero after that exact substitution.

mod build;
mod error;
mod limits;
mod model;
mod projection;

pub(crate) use build::try_single_guard_domain_split;

pub use error::RuleCellError;
pub use limits::RuleCellLimits;
pub use model::{
    FixedIndexRestriction, FixedIndexSpecializationEvidence, ResidualProjectionEvidence,
    ResidualTermDisposition, ResidualTermProjection, RuleCell, RuleCellDomainProof, RuleCellGuard,
    RuleCellTerm, SourceViewBatch, SourceViewConstruction, SourceViewProvenance,
    SymmetrySourceProvenance,
};
pub(crate) use model::{RuleCellGuardDomainProof, RuleCellGuardDomainSplit};

#[cfg(test)]
mod tests;
