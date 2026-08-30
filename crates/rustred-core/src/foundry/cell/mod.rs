//! Proof-bearing application cells refined from exact parametric rules.
//!
//! A cell never authors algebra. It retains the generated rule and immutable
//! translated source views, snapshots the original replay proof domain, and
//! separately proves a tightened or exceptional application box. Every
//! retained pivot guard is proved nonzero as a base-field polynomial over that
//! entire integer box before the cell is installed. Fixed-index specialization
//! uses Symbolica's polynomial substitution; a branch may be pruned only when
//! its coefficient is identically zero after that exact substitution.

mod build;
mod error;
mod limits;
mod model;
mod projection;

pub use error::RuleCellError;
pub use limits::RuleCellLimits;
pub use model::{
    FixedIndexRestriction, ResidualProjectionEvidence, ResidualTermDisposition,
    ResidualTermProjection, RuleCell, RuleCellDomainProof, RuleCellGuard, RuleCellTerm,
    SourceViewBatch, SourceViewConstruction, SourceViewProvenance, SymmetrySourceProvenance,
};

#[cfg(test)]
mod tests;
