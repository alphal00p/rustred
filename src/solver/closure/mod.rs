//! Exact-closure ownership and source-neutral residual-case inventory.
//!
//! This private solver boundary owns the one-way integration between committed
//! publication epochs and the lower case inventory.  Publication children may
//! mint committed exceptional sources; the inventory only retains an opaque
//! source owner and never depends on epoch, handoff, or exact-session types.

pub(crate) mod case_inventory;
mod committed_exceptional_reentry;
mod committed_exceptional_source;
pub(in crate::solver) mod post_ready;
mod publication_handoff;
