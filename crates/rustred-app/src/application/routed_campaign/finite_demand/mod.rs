//! Building blocks for a separate opt-in finite-demand backend.
//! No existing symbolic/v3 execution or checkpoint format is changed here.
#[cfg(all(test, feature = "cli", unix))]
mod diagnostic;
pub(crate) mod backend;
pub(crate) mod storage;
pub(crate) mod trace;
pub(crate) mod prepared;
pub(crate) mod windows;
