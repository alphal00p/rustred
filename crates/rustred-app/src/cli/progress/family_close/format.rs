//! Pure formatting of scalar generation observations.

use super::state::ExactFrame;
use crate::{FamilyCloseGenerationStage, FamilyCloseProgress};

pub(super) fn format_event(event: FamilyCloseProgress, frame: Option<ExactFrame>) -> String {
    use FamilyCloseProgress::*;
    let (elapsed, status) = match event {
        SuccessorGeometry {
            sector,
            snapshot,
            elapsed,
        } => (
            elapsed,
            format!(
                "successors stage={:?} sector={sector:?} location={:?}/{:?}/{:?}/{:?} completed_sectors={} completed_cells={} failed={} complete={} consumed={:?} limits={:?} partition_caps={}/{} max_arity={} partition_requests={} p1={} pn={} pieces={} max_p={} probes={}/{} skipped={} keys={}/{} attempt={:?} telemetry_overflow={} arithmetic_overflow={}",
                snapshot.stage,
                snapshot.sector_ordinal,
                snapshot.rule_ordinal,
                snapshot.rhs_ordinal,
                snapshot.application_ordinal,
                snapshot.completed_sectors,
                snapshot.completed_cells,
                snapshot.failed,
                snapshot.traversal_complete,
                snapshot.consumed,
                snapshot.limits,
                snapshot.max_uncovered_boxes,
                snapshot.max_uncovered_coordinate_cells,
                snapshot.max_arity,
                snapshot.partition_requests,
                snapshot.singleton_partitions,
                snapshot.split_partitions,
                snapshot.total_pieces,
                snapshot.maximum_pieces,
                snapshot.source_degree_probes,
                snapshot.piece_degree_probes,
                snapshot.skipped_singleton_probes,
                snapshot.source_key_builds,
                snapshot.child_key_builds,
                snapshot.failed_attempt,
                snapshot.telemetry_overflow,
                snapshot.arithmetic_overflow
            ),
        ),
        Preparing { arity, elapsed } => (elapsed, format!("preparing K={arity}")),
        Prepared {
            sectors,
            zero_sectors,
            global_zero_sectors,
            elapsed,
        } => (
            elapsed,
            format!(
                "generate {sectors} sectors; {zero_sectors} scoped zero; {global_zero_sectors} global zero proofs"
            ),
        ),
        Generating {
            sector,
            stage,
            elapsed,
            ..
        } => {
            let phase = format_generation_stage(stage, frame);
            (elapsed, format!("sector={sector} {phase}"))
        }
        GeneratedSector {
            sector,
            rules,
            finite_residuals,
            elapsed,
            ..
        } => (
            elapsed,
            format!("sector={sector} generated {rules} rules; {finite_residuals} finite residuals"),
        ),
        FailedSector {
            ordinal,
            sector,
            message,
            elapsed,
        } => {
            // Error strings may contain input text: preserve a single progress
            // line and never interpret terminal controls supplied by it.
            let mut safe_message = String::with_capacity(message.len());
            for c in message.chars() {
                if c.is_control() {
                    safe_message.extend(c.escape_default());
                } else {
                    safe_message.push(c);
                }
            }
            (
                elapsed,
                format!("sector={sector} ordinal={ordinal} FAILED {safe_message}"),
            )
        }
        CheckpointPrepared {
            reused_sectors,
            pending_sectors,
            elapsed,
        } => (
            elapsed,
            format!("checkpoint: {reused_sectors} reused; {pending_sectors} pending"),
        ),
        CheckpointedSector {
            sector,
            bytes,
            elapsed,
            ..
        } => (
            elapsed,
            format!("sector={sector} checkpoint saved ({bytes} bytes)"),
        ),
        CheckingSector {
            sector,
            rules,
            elapsed,
            ..
        } => (elapsed, format!("replay sector={sector} {rules} rules")),
        CheckingRule {
            sector,
            ordinal,
            total,
            elapsed,
        } => (
            elapsed,
            format!("checking sector={sector} rule={}/{total}", ordinal + 1),
        ),
        CheckedSector {
            sector,
            replayed_rules,
            uncovered_boxes,
            issues,
            elapsed,
            ..
        } => (
            elapsed,
            format!(
                "replayed sector={sector} rules={replayed_rules} uncovered={uncovered_boxes} issues={issues}"
            ),
        ),
        LoweringRule {
            sector,
            ordinal,
            total,
            elapsed,
        } => (
            elapsed,
            format!("lowering sector={sector} rule={}/{total}", ordinal + 1),
        ),
        LoweredSector {
            sector,
            cells,
            elapsed,
        } => (elapsed, format!("lowered sector={sector} cells={cells}")),
        Installing {
            sectors,
            rule_cells,
            terminals,
            elapsed,
        } => (
            elapsed,
            format!("installing {sectors} sectors; {rule_cells} cells; {terminals} terminals"),
        ),
        Installed { elapsed } => (elapsed, "installed in memory".into()),
        Encoding { elapsed } => (elapsed, "encoding output".into()),
        Encoded { bytes, elapsed } => (elapsed, format!("encoded {bytes} bytes")),
    };
    format!("RustRed | {:.1}s | {status}", elapsed.as_secs_f64())
}

fn format_generation_stage(stage: FamilyCloseGenerationStage, frame: Option<ExactFrame>) -> String {
    use FamilyCloseGenerationStage::*;
    let detail = match stage {
        Case { pending } => return format!("case; {pending} pending"),
        Discovery { depth, seeds, rows } => {
            return format!("discovery depth={depth} seeds={seeds} rows={rows}");
        }
        Canonicalization => return "canonicalization".into(),
        GuardExtraction => return "guards".into(),
        ExceptionalGeometry => return "exceptional geometry".into(),
        FiniteRetention => return "finite terminal enumeration".into(),
        RuleFound { pending } => return format!("rule found; {pending} pending"),
        Numerical { cases } => return format!("finite search; {cases} cases"),
        ExactMaterialization => "lift".into(),
        ExactFramePrepared { .. } => "frame prepared".into(),
        ExactDenseFractionFreeStarted {
            rows,
            columns,
            reduction_columns,
            rational_coefficients,
        } => format!(
            "dense start rows={rows} cols={columns} reduction_cols={reduction_columns} rational_coefficients={rational_coefficients}"
        ),
        ExactDenseFractionFreeFinished { rank } => format!("dense finish rank={rank}"),
        ExactTargetBlockStarted { columns } => format!("block start cols={columns}"),
        ExactTargetWeightsStarted {
            rows,
            lower_nonzeros,
        } => format!("weights start rows={rows} L_nnz={lower_nonzeros}"),
        ExactTargetWeightsFinished { nonzero_weights } => {
            format!("weights finish nonzero_weights={nonzero_weights}")
        }
        ExactTargetReconstructionStarted { rows, columns } => {
            format!("reconstruction start rows={rows} cols={columns}")
        }
        ExactTargetReconstructionFinished { output_terms } => {
            format!("reconstruction finish output_terms={output_terms}")
        }
        ExactSemiNumericalStarted {
            rows,
            columns,
            variables,
        } => format!("semi start rows={rows} cols={columns} vars={variables}"),
        ExactSemiNumericalCoefficient {
            column,
            probes,
            primes,
        } => {
            format!("semi last completed coefficient col={column} probes={probes} primes={primes}")
        }
        // This is internal generation validation, never saved-source replay or
        // artifact certification. No inner row callbacks are manufactured.
        ExactSemiNumericalExactReplayStarted { support_recovery } => {
            format!("semi exact validation start support_recovery={support_recovery}")
        }
        ExactSemiNumericalExactReplayFinished { output_terms } => match output_terms {
            Some(terms) => format!("semi exact validation finish output_terms={terms}"),
            None => "semi exact validation finish output_terms=none".into(),
        },
        ExactSemiNumericalFinished { output_terms } => {
            format!("semi finish output_terms={output_terms}")
        }
        ExactRowStarted {
            row,
            input_nonzeros,
            reducer_rows,
            reducer_nonzeros,
        } => format!(
            "row={row}/{} start U_rows={reducer_rows} U_nnz={reducer_nonzeros} input_nnz={input_nonzeros}",
            frame.map_or_else(|| "unknown".into(), |frame| frame.source_rows.to_string())
        ),
        ExactRowFinished {
            row,
            accepted_pivot,
            reducer_rows,
            reducer_nonzeros,
        } => format!(
            "row={row}/{} finish U_rows={reducer_rows} U_nnz={reducer_nonzeros} accepted_pivot={accepted_pivot}",
            frame.map_or_else(|| "unknown".into(), |frame| frame.source_rows.to_string())
        ),
    };
    match frame {
        Some(frame) => format!(
            "frame={} exact {detail} source_rows={} integral_cols={} target_col={} vars={}/{} input_terms={}",
            frame
                .sequence
                .map_or_else(|| "unknown".into(), |value| value.to_string()),
            frame.source_rows,
            frame.integral_columns,
            frame.target_column,
            frame.active_variables,
            frame.coefficient_variables,
            frame.input_terms
        ),
        None => format!("frame=unknown exact {detail}"),
    }
}
