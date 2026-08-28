//! Generic sectors, cuts, patterns, and deterministic integral ordering.
//!
//! This module is deliberately independent of loop count, topology, and
//! coefficient fields. A sector is determined exclusively from the
//! **unshifted integer lattice indices**:
//!
//! ```text
//! active i:   n_i >= 1
//! inactive i: n_i <= 0
//! ```
//!
//! Family `PowerShifts` therefore do not appear in any API in this module.
//! Cuts and patterns classify sectors as excluded metadata; they are not zero
//! proofs. Actual zero-sector certificates belong to a later analysis layer.
//!
//! Source correspondence:
//!
//! - LiteRed `jSector` supplies the raw sign convention;
//! - `jSubsectors` supplies active-bit contraction semantics;
//! - `CutDs` and `SectorsPattern` supply independent admissibility filters;
//! - `jComplexity`/`MakeOrderMatrix` motivate the named v1 complexity key.
//!
//! LiteRed permits caller-configurable (and even randomized) order matrices.
//! RustRed instead persists one deterministic policy identifier and exact key
//! schema. Changing that identifier or schema invalidates discovered rules.

mod error;
mod mask;
mod ordering;
mod restriction;

pub use error::Error;
pub use mask::Mask;
pub use ordering::{ComplexityComponent, ComplexityKey, OrderingPolicy, StrictDescentWitness};
pub use restriction::{
    CutConstraint, Exclusion, Pattern, PatternMismatch, PatternSlot, Restrictions,
};

#[cfg(test)]
mod tests;
