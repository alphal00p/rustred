//! Single-field terminal progress and explicitly requested plain stderr logs.

use std::collections::BTreeMap;
use std::io::Write;
use std::time::{Duration, Instant};

use crate::{FamilyCloseGenerationStage, FamilyCloseProgress};

use super::terminal::TerminalSession;

/// Run-local locator and structural counts, never an exact frame identity.
#[derive(Clone, Copy)]
struct ExactFrame {
    sequence: Option<usize>,
    source_rows: usize,
    integral_columns: usize,
    target_column: usize,
    input_terms: usize,
    coefficient_variables: usize,
    active_variables: usize,
}

struct ExactJob {
    /// None remains unknown after overflow; never wrap to a reused locator.
    sequence: Option<usize>,
    frame: Option<ExactFrame>,
}

impl Default for ExactJob {
    fn default() -> Self {
        Self {
            sequence: Some(0),
            frame: None,
        }
    }
}

/// Presentation failures never change artifact generation or output errors.
pub(crate) struct FamilyCloseProgressMonitor<W: Write> {
    terminal: Option<TerminalSession<W>>,
    plain: Option<W>,
    color: bool,
    last_update: Option<Instant>,
    last_was_generating: bool,
    /// Only active jobs; the ordinal is the original manifest/job ordinal.
    exact_jobs: BTreeMap<(usize, u64), ExactJob>,
}

impl<W: Write> FamilyCloseProgressMonitor<W> {
    pub(crate) fn new(writer: W, terminal: bool, force: bool, no_color: bool) -> Self {
        let (terminal, plain) = if terminal {
            (TerminalSession::try_new_line(writer).ok(), None)
        } else {
            (None, force.then_some(writer))
        };
        Self {
            terminal,
            plain,
            color: !no_color,
            last_update: None,
            last_was_generating: false,
            exact_jobs: BTreeMap::new(),
        }
    }

    pub(crate) fn observe(&mut self, event: FamilyCloseProgress) {
        self.observe_at(event, Instant::now());
    }

    fn observe_at(&mut self, event: FamilyCloseProgress, now: Instant) {
        if self.terminal.is_none() && self.plain.is_none() {
            return;
        }
        // Even a suppressed header or transition changes the matching job's
        // frame. This scalar-only update precedes admission; formatting does not.
        self.update_exact_frame(&event);
        let generating = matches!(event, FamilyCloseProgress::Generating { .. });
        // Share one generation throttle across worker sectors and stages:
        // alternating worker phases must not bypass the rate limit. Every
        // major boundary and the first generation event after it stay visible.
        if generating
            && self.last_was_generating
            && self.last_update.is_some_and(|last| {
                now.saturating_duration_since(last) < Duration::from_millis(100)
            })
        {
            return;
        }
        self.last_was_generating = generating;
        let frame = match &event {
            FamilyCloseProgress::Generating {
                ordinal, sector, ..
            } => self
                .exact_jobs
                .get(&(*ordinal, *sector))
                .and_then(|job| job.frame),
            _ => None,
        };
        self.write(&format_event(event, frame), now);
    }

    pub(crate) fn finish(&mut self, success: bool) {
        self.exact_jobs.clear();
        self.write(
            if success {
                "RustRed | output written"
            } else {
                "RustRed | failed (see error)"
            },
            Instant::now(),
        );
        if let Some(terminal) = &mut self.terminal {
            terminal.close();
        }
    }

    fn update_exact_frame(&mut self, event: &FamilyCloseProgress) {
        use FamilyCloseGenerationStage::*;
        match event {
            FamilyCloseProgress::Generating {
                ordinal,
                sector,
                stage,
                ..
            } => {
                let key = (*ordinal, *sector);
                match *stage {
                    ExactFramePrepared {
                        source_rows,
                        integral_columns,
                        target_column,
                        input_terms,
                        coefficient_variables,
                        active_variables,
                    } => {
                        let job = self.exact_jobs.entry(key).or_default();
                        job.sequence = job.sequence.and_then(|value| value.checked_add(1));
                        job.frame = Some(ExactFrame {
                            sequence: job.sequence,
                            source_rows,
                            integral_columns,
                            target_column,
                            input_terms,
                            coefficient_variables,
                            active_variables,
                        });
                    }
                    ExactMaterialization
                    | Case { .. }
                    | Discovery { .. }
                    | Canonicalization
                    | GuardExtraction
                    | ExceptionalGeometry
                    | RuleFound { .. }
                    | Numerical { .. } => {
                        if let Some(job) = self.exact_jobs.get_mut(&key) {
                            job.frame = None;
                        }
                    }
                    ExactDenseFractionFreeStarted { .. }
                    | ExactDenseFractionFreeFinished { .. }
                    | ExactTargetBlockStarted { .. }
                    | ExactTargetWeightsStarted { .. }
                    | ExactTargetWeightsFinished { .. }
                    | ExactTargetReconstructionStarted { .. }
                    | ExactTargetReconstructionFinished { .. }
                    | ExactSemiNumericalStarted { .. }
                    | ExactSemiNumericalCoefficient { .. }
                    | ExactSemiNumericalExactReplayStarted { .. }
                    | ExactSemiNumericalExactReplayFinished { .. }
                    | ExactSemiNumericalFinished { .. }
                    | ExactRowStarted { .. }
                    | ExactRowFinished { .. } => {}
                }
            }
            FamilyCloseProgress::GeneratedSector {
                ordinal, sector, ..
            }
            | FamilyCloseProgress::FailedSector {
                ordinal, sector, ..
            } => {
                self.exact_jobs.remove(&(*ordinal, *sector));
            }
            _ => {}
        }
    }

    fn write(&mut self, text: &str, now: Instant) {
        self.last_update = Some(now);
        if let Some(terminal) = &mut self.terminal {
            if terminal.render_line(text, self.color).is_err() {
                self.terminal = None;
            }
        } else if let Some(writer) = &mut self.plain
            && writeln!(writer, "{text}")
                .and_then(|()| writer.flush())
                .is_err()
        {
            self.plain = None;
        }
    }
}

fn format_event(event: FamilyCloseProgress, frame: Option<ExactFrame>) -> String {
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
        } => format!("semi coefficient col={column} probes={probes} primes={primes}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn generating(
        ordinal: usize,
        sector: u64,
        stage: FamilyCloseGenerationStage,
    ) -> FamilyCloseProgress {
        FamilyCloseProgress::Generating {
            ordinal,
            sector,
            stage,
            elapsed: Duration::ZERO,
        }
    }

    fn prepared_frame(source_rows: usize) -> FamilyCloseGenerationStage {
        FamilyCloseGenerationStage::ExactFramePrepared {
            source_rows,
            integral_columns: source_rows + 3,
            target_column: 2,
            input_terms: source_rows * 7,
            coefficient_variables: 9,
            active_variables: 6,
        }
    }

    fn started_row() -> FamilyCloseGenerationStage {
        FamilyCloseGenerationStage::ExactRowStarted {
            row: 3,
            input_nonzeros: 7,
            reducer_rows: 2,
            reducer_nonzeros: 13,
        }
    }

    #[test]
    fn suppressed_headers_preserve_latest_frame_and_sequence_for_visible_rows() {
        let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        let now = Instant::now();
        monitor.observe_at(
            generating(4, 7, FamilyCloseGenerationStage::ExactMaterialization),
            now,
        );
        monitor.observe_at(generating(4, 7, prepared_frame(11)), now);
        monitor.observe_at(
            generating(4, 7, prepared_frame(19)),
            now + Duration::from_millis(50),
        );
        monitor.observe_at(
            generating(4, 7, started_row()),
            now + Duration::from_millis(100),
        );
        let output = String::from_utf8(monitor.plain.unwrap()).unwrap();
        assert_eq!(output.lines().count(), 2);
        let row = output.lines().last().unwrap();
        assert!(row.contains("sector=7 frame=2 exact row=3/19 start U_rows=2 U_nnz=13"));
        assert!(row.contains("input_nnz=7 source_rows=19 integral_cols=22 target_col=2"));
        assert!(row.contains("vars=6/9 input_terms=133"));
        assert!(!row.contains("source_rows=11"));
    }

    #[test]
    fn interleaved_jobs_keep_separate_frame_context_for_both_key_fields() {
        let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        let now = Instant::now();
        for (ordinal, sector, rows) in [(1, 7, 11), (2, 7, 23), (1, 8, 31)] {
            monitor.observe_at(generating(ordinal, sector, prepared_frame(rows)), now);
        }
        for (tick, ordinal, sector) in [(1, 2, 7), (2, 1, 8), (3, 1, 7)] {
            monitor.observe_at(
                generating(ordinal, sector, started_row()),
                now + Duration::from_millis(tick * 100),
            );
        }
        assert_eq!(monitor.exact_jobs.len(), 3);
        let output = String::from_utf8(monitor.plain.unwrap()).unwrap();
        let rows = output.lines().skip(1).collect::<Vec<_>>();
        assert_eq!(rows.len(), 3);
        for (line, sector, total) in [(rows[0], 7, 23), (rows[1], 8, 31), (rows[2], 7, 11)] {
            assert!(line.contains(&format!("sector={sector} frame=1 exact row=3/{total}")));
            assert!(line.contains(&format!("source_rows={total} ")));
        }
    }

    #[test]
    fn suppressed_coarse_or_nonexact_transitions_invalidate_without_resetting_sequence() {
        use FamilyCloseGenerationStage::*;
        for stage in [
            ExactMaterialization,
            Case { pending: 2 },
            Discovery {
                depth: 1,
                seeds: 3,
                rows: 5,
            },
            Canonicalization,
            GuardExtraction,
            ExceptionalGeometry,
            RuleFound { pending: 1 },
            Numerical { cases: 4 },
        ] {
            let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
            let now = Instant::now();
            monitor.observe_at(generating(4, 7, prepared_frame(11)), now);
            monitor.observe_at(generating(4, 7, stage), now);
            monitor.observe_at(
                generating(4, 7, started_row()),
                now + Duration::from_millis(100),
            );
            monitor.observe_at(
                generating(4, 7, prepared_frame(17)),
                now + Duration::from_millis(101),
            );
            monitor.observe_at(
                generating(4, 7, started_row()),
                now + Duration::from_millis(200),
            );
            let output = String::from_utf8(monitor.plain.unwrap()).unwrap();
            let lines = output.lines().collect::<Vec<_>>();
            assert_eq!(lines.len(), 3);
            assert!(
                lines[1].contains("frame=unknown exact row=3/unknown"),
                "{stage:?}"
            );
            assert!(!lines[1].contains("source_rows="), "{stage:?}");
            assert!(lines[2].contains("frame=2 exact row=3/17"), "{stage:?}");
        }
    }

    #[test]
    fn missing_header_and_disabled_output_do_not_create_frame_context() {
        let now = Instant::now();
        let mut plain = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        plain.observe_at(generating(4, 7, started_row()), now);
        assert!(plain.exact_jobs.is_empty());
        let output = String::from_utf8(plain.plain.unwrap()).unwrap();
        assert!(output.contains("frame=unknown exact row=3/unknown"));

        let mut quiet = FamilyCloseProgressMonitor::new(Vec::new(), false, false, true);
        quiet.observe_at(generating(4, 7, prepared_frame(11)), now);
        quiet.observe_at(generating(4, 7, started_row()), now);
        assert!(quiet.exact_jobs.is_empty());
        assert!(quiet.last_update.is_none());
        assert!(quiet.plain.is_none() && quiet.terminal.is_none());
    }

    #[test]
    fn completed_failed_and_finished_jobs_release_cached_frames() {
        let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        let now = Instant::now();
        for ordinal in 1..=3 {
            monitor.observe_at(generating(ordinal, 7, prepared_frame(11)), now);
        }
        monitor.observe_at(
            FamilyCloseProgress::GeneratedSector {
                ordinal: 1,
                sector: 7,
                rules: 1,
                finite_residuals: 0,
                elapsed: Duration::ZERO,
            },
            now,
        );
        assert_eq!(monitor.exact_jobs.len(), 2);
        assert!(!monitor.exact_jobs.contains_key(&(1, 7)));
        monitor.observe_at(
            FamilyCloseProgress::FailedSector {
                ordinal: 2,
                sector: 7,
                message: "test failure".into(),
                elapsed: Duration::ZERO,
            },
            now,
        );
        assert_eq!(monitor.exact_jobs.len(), 1);
        assert!(monitor.exact_jobs.contains_key(&(3, 7)));
        monitor.finish(false);
        assert!(monitor.exact_jobs.is_empty());
    }

    #[test]
    fn frame_sequence_overflow_stays_unknown_but_retains_current_dimensions() {
        let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        monitor.exact_jobs.insert(
            (4, 7),
            ExactJob {
                sequence: Some(usize::MAX),
                frame: None,
            },
        );
        let now = Instant::now();
        for (tick, total) in [(0, 11), (1, 17)] {
            monitor.observe_at(
                generating(4, 7, prepared_frame(total)),
                now + Duration::from_millis(tick * 100),
            );
        }
        assert!(monitor.exact_jobs[&(4, 7)].sequence.is_none());
        let output = String::from_utf8(monitor.plain.unwrap()).unwrap();
        assert_eq!(output.lines().count(), 2);
        for (line, total) in output.lines().zip([11, 17]) {
            assert!(line.contains("frame=unknown exact frame prepared"));
            assert!(line.contains(&format!("source_rows={total} ")));
            assert!(!line.contains("frame=0"));
        }
    }

    #[test]
    fn every_exact_stage_has_plain_scalar_details_without_certification_claims() {
        use FamilyCloseGenerationStage::*;
        let frame = Some(ExactFrame {
            sequence: Some(4),
            source_rows: 11,
            integral_columns: 14,
            target_column: 2,
            input_terms: 77,
            coefficient_variables: 9,
            active_variables: 6,
        });
        for (stage, expected) in [
            (prepared_frame(11), "frame prepared"),
            (
                ExactDenseFractionFreeStarted {
                    rows: 13,
                    columns: 19,
                    reduction_columns: 4,
                    rational_coefficients: true,
                },
                "dense start rows=13 cols=19 reduction_cols=4 rational_coefficients=true",
            ),
            (
                ExactDenseFractionFreeFinished { rank: 9 },
                "dense finish rank=9",
            ),
            (ExactTargetBlockStarted { columns: 4 }, "block start cols=4"),
            (
                ExactTargetWeightsStarted {
                    rows: 13,
                    lower_nonzeros: 29,
                },
                "weights start rows=13 L_nnz=29",
            ),
            (
                ExactTargetWeightsFinished { nonzero_weights: 6 },
                "weights finish nonzero_weights=6",
            ),
            (
                ExactTargetReconstructionStarted {
                    rows: 13,
                    columns: 19,
                },
                "reconstruction start rows=13 cols=19",
            ),
            (
                ExactTargetReconstructionFinished { output_terms: 23 },
                "reconstruction finish output_terms=23",
            ),
            (
                ExactSemiNumericalStarted {
                    rows: 13,
                    columns: 19,
                    variables: 7,
                },
                "semi start rows=13 cols=19 vars=7",
            ),
            (
                ExactSemiNumericalCoefficient {
                    column: 3,
                    probes: 11,
                    primes: 2,
                },
                "semi coefficient col=3 probes=11 primes=2",
            ),
            (
                ExactSemiNumericalExactReplayStarted {
                    support_recovery: true,
                },
                "semi exact validation start support_recovery=true",
            ),
            (
                ExactSemiNumericalExactReplayFinished {
                    output_terms: Some(23),
                },
                "semi exact validation finish output_terms=23",
            ),
            (
                ExactSemiNumericalExactReplayFinished { output_terms: None },
                "semi exact validation finish output_terms=none",
            ),
            (
                ExactSemiNumericalFinished { output_terms: 23 },
                "semi finish output_terms=23",
            ),
            (
                started_row(),
                "row=3/11 start U_rows=2 U_nnz=13 input_nnz=7",
            ),
            (
                ExactRowFinished {
                    row: 3,
                    accepted_pivot: true,
                    reducer_rows: 3,
                    reducer_nonzeros: 17,
                },
                "row=3/11 finish U_rows=3 U_nnz=17 accepted_pivot=true",
            ),
            (
                ExactRowFinished {
                    row: 3,
                    accepted_pivot: false,
                    reducer_rows: 2,
                    reducer_nonzeros: 13,
                },
                "row=3/11 finish U_rows=2 U_nnz=13 accepted_pivot=false",
            ),
        ] {
            let output = format_event(generating(4, 7, stage), frame);
            assert!(output.contains("sector=7 frame=4 exact "), "{output}");
            assert!(output.contains(expected), "{output}");
            assert!(
                output.ends_with(
                    "source_rows=11 integral_cols=14 target_col=2 vars=6/9 input_terms=77"
                )
            );
            for forbidden in [
                "checking",
                "install",
                "closed",
                "certified",
                "replay",
                "target solved",
            ] {
                assert!(!output.contains(forbidden), "{output}");
            }
            assert!(!output.contains('\u{1b}') && !output.contains('\r') && !output.contains('\n'));
        }
    }

    #[test]
    fn redirected_progress_is_opt_in_and_plain() {
        let event = FamilyCloseProgress::Preparing {
            arity: 3,
            elapsed: Duration::ZERO,
        };
        let mut quiet = FamilyCloseProgressMonitor::new(Vec::new(), false, false, false);
        quiet.observe(event.clone());
        assert!(quiet.plain.is_none() && quiet.terminal.is_none());
        let mut plain = FamilyCloseProgressMonitor::new(Vec::new(), false, true, false);
        plain.observe(event);
        plain.finish(true);
        let output = String::from_utf8(plain.plain.unwrap()).unwrap();
        assert!(output.contains("preparing K=3"));
        assert!(output.contains("output written"));
        assert!(!output.contains('\u{1b}') && !output.contains('\r'));
    }

    #[test]
    fn replay_observation_is_not_formatted_as_publication() {
        let output = format_event(
            FamilyCloseProgress::CheckedSector {
                ordinal: 0,
                sector: 7,
                replayed_rules: 2,
                uncovered_boxes: 1,
                issues: 1,
                elapsed: Duration::ZERO,
            },
            None,
        );
        assert!(output.contains("uncovered=1 issues=1"));
        assert!(!output.contains("closed") && !output.contains("written"));
    }

    #[test]
    fn failures_bypass_generation_throttle_and_escape_terminal_controls() {
        let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        let now = Instant::now();
        monitor.observe_at(
            FamilyCloseProgress::Generating {
                ordinal: 7,
                sector: 5,
                stage: FamilyCloseGenerationStage::ExactMaterialization,
                elapsed: Duration::ZERO,
            },
            now,
        );
        for ordinal in [7, 8] {
            monitor.observe_at(
                FamilyCloseProgress::FailedSector {
                    ordinal,
                    sector: 5,
                    message: "output: quota\ninvalid\r\u{1b}[2J".into(),
                    elapsed: Duration::ZERO,
                },
                now,
            );
        }
        let output = String::from_utf8(monitor.plain.unwrap()).unwrap();
        assert_eq!(output.lines().count(), 3);
        assert!(output.contains("ordinal=7 FAILED output: quota\\ninvalid\\r\\u{1b}[2J"));
        assert!(output.contains("ordinal=8 FAILED"));
        assert!(!output.contains('\u{1b}') && !output.contains('\r'));
    }

    #[test]
    fn consecutive_rule_checks_are_visible_inside_the_throttle_window() {
        let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        monitor.observe(FamilyCloseProgress::CheckingSector {
            ordinal: 0,
            sector: 214,
            rules: 161,
            elapsed: Duration::ZERO,
        });
        for ordinal in [0, 1, 160] {
            // Reset the last-render timestamp explicitly: the assertion must
            // not depend on how quickly this test process is scheduled.
            monitor.last_update = Some(Instant::now());
            monitor.observe(FamilyCloseProgress::CheckingRule {
                sector: 214,
                ordinal,
                total: 161,
                elapsed: Duration::ZERO,
            });
        }
        let output = String::from_utf8(monitor.plain.unwrap()).unwrap();
        assert_eq!(output.lines().count(), 4);
        for expected in ["rule=1/161", "rule=2/161", "rule=161/161"] {
            assert!(output.contains(expected), "missing {expected}: {output}");
        }
        assert!(!output.contains("closed") && !output.contains("written"));
    }

    #[test]
    fn alternating_worker_stages_share_one_generation_throttle() {
        let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        let now = Instant::now();
        let event = |ordinal: usize| FamilyCloseProgress::Generating {
            ordinal: ordinal % 6,
            sector: (ordinal % 6) as u64,
            stage: match ordinal % 8 {
                0 => FamilyCloseGenerationStage::Case { pending: ordinal },
                1 => FamilyCloseGenerationStage::Discovery {
                    depth: 1,
                    seeds: ordinal,
                    rows: ordinal,
                },
                2 => FamilyCloseGenerationStage::ExactMaterialization,
                3 => FamilyCloseGenerationStage::GuardExtraction,
                4 => prepared_frame(11),
                5 => started_row(),
                6 => FamilyCloseGenerationStage::ExactTargetWeightsStarted {
                    rows: 11,
                    lower_nonzeros: 17,
                },
                _ => FamilyCloseGenerationStage::ExactSemiNumericalCoefficient {
                    column: 2,
                    probes: 3,
                    primes: 1,
                },
            },
            elapsed: Duration::ZERO,
        };
        monitor.observe_at(
            FamilyCloseProgress::Prepared {
                sectors: 6,
                zero_sectors: 0,
                global_zero_sectors: 0,
                elapsed: Duration::ZERO,
            },
            now,
        );
        for ordinal in 0..10_000 {
            monitor.observe_at(event(ordinal), now);
        }
        assert_eq!(
            monitor
                .plain
                .as_ref()
                .unwrap()
                .iter()
                .filter(|&&b| b == b'\n')
                .count(),
            2,
        );
        monitor.observe_at(event(1), now + Duration::from_millis(99));
        assert_eq!(
            monitor
                .plain
                .as_ref()
                .unwrap()
                .iter()
                .filter(|&&b| b == b'\n')
                .count(),
            2,
        );
        let next_tick = now + Duration::from_millis(100);
        monitor.observe_at(event(2), next_tick);
        monitor.observe_at(
            FamilyCloseProgress::GeneratedSector {
                ordinal: 0,
                sector: 0,
                rules: 1,
                finite_residuals: 1,
                elapsed: Duration::ZERO,
            },
            next_tick,
        );
        // A major boundary is followed immediately by one visible generation
        // event, even when another worker is already inside the same tick.
        monitor.observe_at(event(3), next_tick);
        monitor.observe_at(event(4), next_tick);
        let output = String::from_utf8(monitor.plain.unwrap()).unwrap();
        assert_eq!(output.lines().count(), 5);
        assert!(output.contains("exact lift"));
        assert!(output.contains("generated 1 rules"));
        assert!(output.contains("sector=3 guards"));
    }

    #[test]
    fn adjacent_phase_boundaries_are_never_throttled() {
        let mut monitor = FamilyCloseProgressMonitor::new(Vec::new(), false, true, true);
        for event in [
            FamilyCloseProgress::GeneratedSector {
                ordinal: 0,
                sector: 7,
                rules: 2,
                finite_residuals: 1,
                elapsed: Duration::ZERO,
            },
            FamilyCloseProgress::CheckingSector {
                ordinal: 0,
                sector: 7,
                rules: 2,
                elapsed: Duration::ZERO,
            },
            FamilyCloseProgress::CheckingRule {
                sector: 7,
                ordinal: 0,
                total: 2,
                elapsed: Duration::ZERO,
            },
            FamilyCloseProgress::CheckedSector {
                ordinal: 0,
                sector: 7,
                replayed_rules: 2,
                uncovered_boxes: 0,
                issues: 0,
                elapsed: Duration::ZERO,
            },
            FamilyCloseProgress::LoweringRule {
                sector: 7,
                ordinal: 0,
                total: 2,
                elapsed: Duration::ZERO,
            },
            FamilyCloseProgress::LoweredSector {
                sector: 7,
                cells: 2,
                elapsed: Duration::ZERO,
            },
        ] {
            monitor.observe(event);
        }
        let output = String::from_utf8(monitor.plain.unwrap()).unwrap();
        for expected in [
            "generated 2",
            "replay sector=7",
            "checking sector=7 rule=1/2",
            "replayed sector=7",
            "lowering sector=7 rule=1/2",
            "lowered sector=7",
        ] {
            assert!(output.contains(expected), "missing {expected}: {output}");
        }
    }
}
