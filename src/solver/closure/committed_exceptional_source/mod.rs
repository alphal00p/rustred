//! Sealed integration boundary for committed exceptional case sources.
//!
//! The generic protocol is independent of case-inventory and epoch types. Its
//! sibling adapter is the only port implementation and the sole concrete
//! committed-epoch ingress into authority assembly.

pub(in crate::solver::closure) mod epoch_adapter;
pub(in crate::solver::closure) mod protocol;
