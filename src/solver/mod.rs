//! Internal solver implementations.
//!
//! Public orchestration belongs to the stable RustRed API.  Solver ownership,
//! transactional state, and exact replay machinery remain crate-private so
//! their invariants cannot be bypassed by callers.

pub(crate) mod closure;
pub(crate) mod exact_session;
