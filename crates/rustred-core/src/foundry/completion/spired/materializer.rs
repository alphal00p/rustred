//! Exact materialization boundary for targeted completion hits.
//!
//! A materializer receives only a checked modular hit and the exhaustive
//! target partition bound to the same physical frame. Discovery traces and
//! raw row ordinals cannot enter this boundary directly. The resulting exact
//! circuit still carries relation authority only after the existing full
//! source replay succeeds.

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::frame::exact::{
    ExactCircuitError, ExactCircuitLift, ExactCircuitLimits, RootedExactCircuitLift,
    try_lift_exact_circuit, try_lift_rooted_exact_circuit,
};
use crate::foundry::completion::frame::modular::ModularHit;
use crate::foundry::completion::stratum::TargetColumnPartition;

/// Backend seam for recovering and replaying one exact target rule.
///
/// [`ExactCircuitLift::ModularSupportDidNotLift`] is an ordinary inconclusive
/// result, not an infrastructure error. Implementations must preserve that
/// distinction and must not turn bounded discovery evidence into closure
/// authority.
pub(crate) trait TargetRuleMaterializer {
    fn try_materialize<'frame>(
        &self,
        context: &IndexedCoefficientContext,
        hit: &ModularHit<'frame>,
        partition: &TargetColumnPartition<'frame>,
        limits: ExactCircuitLimits,
    ) -> Result<ExactCircuitLift, ExactCircuitError>;

    /// Recover a relation which is constrained to use one designated later
    /// source row after all of its retained predecessors.
    ///
    /// This is the exact counterpart of the post-hit GPLU trace.  The trace
    /// itself remains scheduling evidence: implementations must cancel the
    /// forbidden block exactly, prove a nonzero target coefficient, and pass
    /// the common full-source replay before returning authority.
    fn try_materialize_rooted<'frame>(
        &self,
        context: &IndexedCoefficientContext,
        hit: &ModularHit<'frame>,
        partition: &TargetColumnPartition<'frame>,
        predecessor_rows: &[usize],
        root_frame_row: usize,
        limits: ExactCircuitLimits,
    ) -> Result<RootedExactCircuitLift, ExactCircuitError>;
}

/// Exact Symbolica materialization over the independent support selected by
/// the checked modular hit.
///
/// This adapter deliberately does not invoke the complete-frame fallback.
/// Callers therefore retain the existing typed, inconclusive support-miss
/// outcome and decide any separately bounded fallback policy themselves.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PrunedExactMaterializer;

impl TargetRuleMaterializer for PrunedExactMaterializer {
    fn try_materialize<'frame>(
        &self,
        context: &IndexedCoefficientContext,
        hit: &ModularHit<'frame>,
        partition: &TargetColumnPartition<'frame>,
        limits: ExactCircuitLimits,
    ) -> Result<ExactCircuitLift, ExactCircuitError> {
        try_lift_exact_circuit(context, hit, partition, limits)
    }

    fn try_materialize_rooted<'frame>(
        &self,
        context: &IndexedCoefficientContext,
        hit: &ModularHit<'frame>,
        partition: &TargetColumnPartition<'frame>,
        predecessor_rows: &[usize],
        root_frame_row: usize,
        limits: ExactCircuitLimits,
    ) -> Result<RootedExactCircuitLift, ExactCircuitError> {
        try_lift_rooted_exact_circuit(
            context,
            hit,
            partition,
            predecessor_rows,
            root_frame_row,
            limits,
        )
    }
}
