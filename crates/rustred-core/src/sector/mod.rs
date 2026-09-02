//! Generic sector classification, ordering, and exact symmetry verification.
//!
//! The mask, restriction, and ordering foundation is independent of loop
//! count, topology, and coefficient fields. A sector is determined
//! exclusively from the **unshifted integer lattice indices**:
//!
//! ```text
//! active i:   n_i >= 1
//! inactive i: n_i <= 0
//! ```
//!
//! Family `PowerShifts` therefore do not affect the mask, restriction, or
//! ordering foundation. Cuts and patterns classify sectors as excluded
//! metadata; they are not zero proofs. [`zero`] owns the separate
//! family-aware analytic zero test, while [`symmetry`] consumes authenticated
//! families and exact Symbolica coefficients to verify caller-supplied
//! momentum maps. Neither analysis generates topology-authored candidates.
//!
//! Source correspondence:
//!
//! - LiteRed `jSector` supplies the raw sign convention;
//! - `jSubsectors` supplies active-bit contraction semantics;
//! - `CutDs` and `SectorsPattern` supply independent admissibility filters;
//! - `jComplexity`/`MakeOrderMatrix` motivate the named v1 complexity key.
//!
//! LiteRed permits caller-configurable (and even randomized) order matrices.
//! RustRed instead persists an exact deterministic policy identity and key
//! display. Its optional coordinate priority is a complete, versioned
//! permutation—not a transient portfolio ordinal. Changing that identity or
//! serialized key invalidates discovered rules.

mod error;
mod interior;
mod mask;
mod monotone;
mod ordering;
mod restriction;
mod shift_ordering;
pub mod symmetry;
pub mod zero;

pub use error::Error;
pub use interior::{InteriorBounds, SectorInteriorDomain};
pub use mask::Mask;
pub use monotone::{
    ActivePinchThreshold, SectorMonotoneDomain, SectorMonotonePointClass,
    SectorMonotoneShiftDescentWitness, SectorMonotoneTargetCell, SectorMonotoneTargetCellKind,
    SectorMonotoneTargetCells, SectorMonotoneTargetPartition, SectorMonotoneTargetPartitionCensus,
};
pub use ordering::{
    ComplexityComponent, ComplexityKey, CoordinatePriority, CoordinatePriorityError,
    CoordinatePriorityLimits, CoordinatePriorityOrderingV1, MAX_PACKED_ORDERING_PRIORITY_ARITY,
    OrderingPolicy, OrderingPolicyStableId, StrictDescentWitness,
};
pub use restriction::{
    CutConstraint, Exclusion, Pattern, PatternMismatch, PatternSlot, Restrictions,
};
pub use shift_ordering::{ShiftComplexityKey, ShiftStrictDescentWitness};

#[cfg(test)]
mod tests;
