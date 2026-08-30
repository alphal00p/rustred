//! Bounded reduced ordered decision DAGs over semantic guard atoms.
//!
//! The compiler selects the first candidate whose grouped coefficient ideals
//! are all on their nonzero branches. Candidate leaves are discovery routing
//! results only: they are neither RuleCell owners nor closure terminals.

mod build;
mod error;
mod limits;
mod model;

pub(crate) use error::GuardDecisionDagError;
pub(crate) use limits::GuardDecisionDagLimits;
pub(crate) use model::{
    CoefficientIdealGuardDag, GuardDecisionCandidate, GuardDecisionCandidateId,
    GuardDecisionDagStats, GuardDecisionOutcome,
};

#[cfg(test)]
mod tests;
