//! Explicit scalar schema conversion, isolated from solver event definitions.
use super::StageDetail;
use crate::FamilyCloseGenerationStage as G;

impl From<G> for StageDetail {
    fn from(stage: G) -> Self {
        match stage {
            G::Case { pending } => Self::Case { pending },
            G::Discovery { depth, seeds, rows } => Self::Discovery { depth, seeds, rows },
            G::ExactMaterialization => Self::ExactMaterialization,
            G::ExactFramePrepared {
                source_rows,
                integral_columns,
                target_column,
                input_terms,
                coefficient_variables,
                active_variables,
            } => Self::ExactFramePrepared {
                source_rows,
                integral_columns,
                target_column,
                input_terms,
                coefficient_variables,
                active_variables,
            },
            G::ExactDenseFractionFreeStarted {
                rows,
                columns,
                reduction_columns,
                rational_coefficients,
            } => Self::ExactDenseFractionFreeStarted {
                rows,
                columns,
                reduction_columns,
                rational_coefficients,
            },
            G::ExactDenseFractionFreeFinished { rank } => {
                Self::ExactDenseFractionFreeFinished { rank }
            }
            G::ExactTargetBlockStarted { columns } => Self::ExactTargetBlockStarted { columns },
            G::ExactTargetWeightsStarted {
                rows,
                lower_nonzeros,
            } => Self::ExactTargetWeightsStarted {
                rows,
                lower_nonzeros,
            },
            G::ExactTargetWeightsFinished { nonzero_weights } => {
                Self::ExactTargetWeightsFinished { nonzero_weights }
            }
            G::ExactTargetReconstructionStarted { rows, columns } => {
                Self::ExactTargetReconstructionStarted { rows, columns }
            }
            G::ExactTargetReconstructionFinished { output_terms } => {
                Self::ExactTargetReconstructionFinished { output_terms }
            }
            G::ExactSemiNumericalStarted {
                rows,
                columns,
                variables,
            } => Self::ExactSemiNumericalStarted {
                rows,
                columns,
                variables,
            },
            G::ExactSemiNumericalCoefficient {
                column,
                probes,
                primes,
            } => Self::ExactSemiNumericalCoefficient {
                column,
                probes,
                primes,
            },
            G::ExactSemiNumericalExactReplayStarted { support_recovery } => {
                Self::ExactSemiNumericalExactReplayStarted { support_recovery }
            }
            G::ExactSemiNumericalExactReplayFinished { output_terms } => {
                Self::ExactSemiNumericalExactReplayFinished { output_terms }
            }
            G::ExactSemiNumericalFinished { output_terms } => {
                Self::ExactSemiNumericalFinished { output_terms }
            }
            G::ExactRowStarted {
                row,
                input_nonzeros,
                reducer_rows,
                reducer_nonzeros,
            } => Self::ExactRowStarted {
                row,
                input_nonzeros,
                reducer_rows,
                reducer_nonzeros,
            },
            G::ExactRowFinished {
                row,
                accepted_pivot,
                reducer_rows,
                reducer_nonzeros,
            } => Self::ExactRowFinished {
                row,
                accepted_pivot,
                reducer_rows,
                reducer_nonzeros,
            },
            G::Canonicalization => Self::Canonicalization,
            G::GuardExtraction => Self::GuardExtraction,
            G::ExceptionalGeometry => Self::ExceptionalGeometry,
            G::FiniteRetention => Self::FiniteRetention,
            G::RuleTrialFinished {
                trial,
                outcome,
                search_seeds,
                search_rows,
                independent_rows,
                exact_trace_rows,
                exact_trace_terms,
                exact_lifts,
                guard_branches,
                geometry_calls,
                search_us,
                exact_materialization_us,
                guard_extraction_us,
                geometry_us,
            } => Self::RuleTrialFinished {
                trial,
                outcome: outcome.into(),
                search_seeds,
                search_rows,
                independent_rows,
                exact_trace_rows,
                exact_trace_terms,
                exact_lifts,
                guard_branches,
                geometry_calls,
                search_us: search_us.min(u128::from(u64::MAX)) as u64,
                exact_materialization_us: exact_materialization_us.min(u128::from(u64::MAX)) as u64,
                guard_extraction_us: guard_extraction_us.min(u128::from(u64::MAX)) as u64,
                geometry_us: geometry_us.min(u128::from(u64::MAX)) as u64,
                duration_micros_saturated: [
                    search_us,
                    exact_materialization_us,
                    guard_extraction_us,
                    geometry_us,
                ]
                .into_iter()
                .any(|value| value > u128::from(u64::MAX)),
            },
            G::RuleFound { pending } => Self::RuleFound { pending },
            G::Numerical { cases } => Self::Numerical { cases },
        }
    }
}
