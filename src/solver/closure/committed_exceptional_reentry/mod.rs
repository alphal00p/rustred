//! One-shot orchestration for committed exceptional-domain re-entry.
//!
//! The capability leaf seals the single shallow authority copy retained by a
//! fresh child. The fresh-session sibling owns construction above both the
//! concrete publication-epoch source and its source-neutral inventory adapter.

mod capability;
mod fresh_session;

pub(in crate::solver::closure) use capability::CommittedExceptionalAuthorityCopyPermit;
pub(in crate::solver::closure) use fresh_session::{
    CommittedExceptionalFreshSessionBuildError, CommittedExceptionalFreshSessionLimits,
    try_build_fresh_exact_session_for_admitted_transform,
};
