//! Exact generated-affine solve-session foundations.
//!
//! This boundary owns topology-neutral physical coordinates, immutable solve
//! plans, sealed exact rows, and authority-free GMP recentering.  The
//! transactional database and closure scheduler are migrated here in later
//! restructuring tranches; until then they consume this deliberately narrow
//! crate-private facade.

mod physical_key;
mod physical_row;
mod plan;
mod recenter;

pub(crate) use physical_key::{
    GeneratedAffineResidualGroupLatticeShift, GeneratedAffineResidualGroupPhysicalFrame,
    GeneratedAffineResidualGroupPhysicalKey,
    GeneratedAffineResidualGroupPhysicalKeyComparisonComponent,
    GeneratedAffineResidualGroupPhysicalKeyComparisonWitness,
    GeneratedAffineResidualGroupPhysicalKeyError, GeneratedAffineResidualGroupPhysicalKeyLimits,
};
pub(crate) use physical_row::{
    GeneratedAffineResidualGroupExactPhysicalRow,
    GeneratedAffineResidualGroupReplayedExactPhysicalRow,
};
pub(crate) use plan::{
    GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolvePlanError,
    GeneratedAffineResidualGroupSolvePlanLimits, GeneratedAffineResidualGroupSolvePlanReplayLimits,
    GeneratedAffineResidualGroupSolveTargetLocator,
};
pub(crate) use recenter::{
    ExactRecenterKernelError, ExactRecenterKernelLimits, ExactRecenterKernelStats,
    ExactRecenteredApplicationRow, ExactRecenteredRow, ExactRecenteredTerm, ExactTargetOffset,
    admit_inert_owner, bounded_add, checked_add, exact_offsets_equal, execute_target_offset,
    integer_bits, observe_inert_owner, preflight_exact_geometry, prospective_integer_heap_bytes,
    translate_centered_row, verify_target_offset_census,
};

#[cfg(test)]
pub(crate) use physical_key::{
    GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V2_SCHEMA,
    GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V3_SCHEMA,
};
#[cfg(test)]
pub(crate) use physical_row::{
    GeneratedAffineResidualGroupExactPhysicalRowCompiler,
    GeneratedAffineResidualGroupExactPhysicalRowLimits,
};
#[cfg(test)]
pub(crate) use plan::GENERATED_AFFINE_RESIDUAL_GROUP_SOLVE_PLAN_V3_SCHEMA;
#[cfg(test)]
pub(crate) use recenter::{
    centered_shift_arithmetic_operations_for_test,
    reset_centered_shift_arithmetic_operations_for_test,
};
