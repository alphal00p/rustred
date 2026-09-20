use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::FamilyCloseProgressMonitor;
use super::format::format_event;
use super::presenter::{Display, Outcome, progress_bar};
use super::resources::Resources;
use super::state::{ExactFrame, ExactJob, Tracker};
use crate::{FamilyCloseGenerationStage, FamilyCloseProgress};

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

#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Buffer {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
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
            "semi last completed coefficient col=3 probes=11 primes=2",
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
            output
                .ends_with("source_rows=11 integral_cols=14 target_col=2 vars=6/9 input_terms=77")
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
fn coalesced_frames_preserve_dimensions_and_sequence_overflow_stays_unknown() {
    let mut tracker = Tracker::default();
    let now = Instant::now();
    tracker.observe(generating(4, 7, prepared_frame(11)), now);
    tracker.observe(generating(4, 7, prepared_frame(19)), now);
    let snapshot = tracker.observe(generating(4, 7, started_row()), now);
    let line = format_event(snapshot.event, snapshot.frame);
    assert!(line.contains("sector=7 frame=2 exact row=3/19 start U_rows=2 U_nnz=13"));
    assert!(line.contains("input_nnz=7 source_rows=19 integral_cols=22 target_col=2"));
    assert!(line.contains("vars=6/9 input_terms=133"));
    tracker.jobs.insert(
        (4, 7),
        ExactJob {
            sequence: Some(usize::MAX),
            case: Some(1),
            frame: None,
        },
    );
    for rows in [11, 17] {
        let snapshot = tracker.observe(generating(4, 7, prepared_frame(rows)), now);
        let frame = snapshot.frame.unwrap();
        assert_eq!(frame.sequence, None);
        assert_eq!(frame.source_rows, rows);
    }
}

#[test]
fn every_coarse_transition_invalidates_old_frame_without_reusing_its_sequence() {
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
        let mut tracker = Tracker::default();
        let now = Instant::now();
        tracker.observe(generating(4, 7, prepared_frame(11)), now);
        tracker.observe(generating(4, 7, stage), now);
        assert!(
            tracker
                .observe(generating(4, 7, started_row()), now)
                .frame
                .is_none()
        );
        let snapshot = tracker.observe(generating(4, 7, prepared_frame(17)), now);
        assert_eq!(snapshot.frame.unwrap().sequence, Some(2));
    }
}

#[test]
fn rule_hits_are_live_observations_not_completed_sector_totals() {
    let mut tracker = Tracker::default();
    let now = Instant::now();
    for _ in 0..192 {
        tracker.observe(
            generating(4, 7, FamilyCloseGenerationStage::RuleFound { pending: 2 }),
            now,
        );
    }
    let snapshot = tracker.observe(generating(4, 7, started_row()), now);
    assert_eq!(snapshot.counts.observed_rules, 192);
    assert_eq!(snapshot.counts.rules, 0);
    let display = Display::new(Some(&snapshot), Resources::default(), now, now, None);
    assert!(display.counts.contains("completed-sector totals: rules=0"));
    assert!(display.counts.contains("observed rule hits=192"));
}

#[test]
fn stale_detail_clock_is_distinct_from_live_elapsed_and_quiet_age() {
    let started = Instant::now();
    let mut tracker = Tracker::default();
    let snapshot = tracker.observe(generating(4, 7, started_row()), started);
    let later = started + Duration::from_secs(12);
    let display = Display::new(Some(&snapshot), Resources::default(), started, later, None);
    assert!(display.header.contains("elapsed=12.0s quiet=12.0s"));
    assert!(display.detail.starts_with("last event: RustRed | 0.0s |"));
    assert!(display.footer.contains("age=12.0s"));
    assert!(
        display
            .resources
            .contains("RSS=unavailable peak=unavailable")
    );
    assert!(display.resources.contains("CPU=unavailable"));
}

#[test]
fn sector_bar_does_not_invent_denominators_or_overflow() {
    assert!(progress_bar(None, Duration::ZERO).contains("total unknown (no ETA)"));
    assert!(progress_bar(Some((0, 0)), Duration::ZERO).contains("no sectors scheduled"));
    assert!(progress_bar(Some((1, 0)), Duration::ZERO).contains("total unknown"));
    let maximum = progress_bar(Some((usize::MAX, usize::MAX)), Duration::ZERO);
    assert!(maximum.contains("[####################]"));
    assert!(maximum.contains("not closure"));
}

#[test]
fn event_storm_cannot_bypass_plain_heartbeat_rate_limit() {
    let buffer = Buffer::default();
    let mut monitor = FamilyCloseProgressMonitor::with_test_settings(
        buffer.clone(),
        false,
        true,
        true,
        Duration::from_secs(60),
        None,
    );
    for ordinal in 0..10_000 {
        monitor.observe(generating(
            ordinal % 6,
            (ordinal % 6) as u64,
            FamilyCloseGenerationStage::RuleFound { pending: ordinal },
        ));
    }
    monitor.finish(true);
    let text = buffer.text();
    assert!(
        (1..=2).contains(&text.lines().count()),
        "{} records",
        text.lines().count()
    );
    assert!(text.contains("observed rule hits=10000"));
    assert!(text.contains("output written"));
    assert!(!text.contains('\u{1b}') && !text.contains('\r'));
}

#[test]
fn narrow_and_resized_tty_is_bounded_and_no_color_preserves_cleanup() {
    use super::super::terminal::TerminalSession;
    let buffer = Buffer::default();
    {
        let mut terminal = TerminalSession::try_new_family_fixed(buffer.clone(), 80).unwrap();
        for width in [80, 16, 1, 120] {
            terminal.resize_family_fixed(width).unwrap();
            terminal
                .render_family(
                    [
                        "RustRed | running",
                        "sector generation [###---] 1/2",
                        "totals",
                        "last event with a very long exact source frame and coefficients detail",
                        "process RSS=1.0 MiB peak=2.0 MiB",
                        "phase=exact sparse elimination",
                    ],
                    false,
                    false,
                )
                .unwrap();
        }
        terminal.close();
        terminal.close();
    }
    let text = buffer.text();
    assert!(text.contains("RustRed"));
    assert!(text.contains("\u{1b}[?25h"));
    assert!(text.ends_with("\u{1b}[1E"));
    assert!(!text.contains("\u{1b}[38;5;") && !text.contains("\u{1b}[48;5;"));
    assert!(!text.contains("\u{1b}[1m"));
}

#[test]
fn unwind_reports_unconfirmed_output_and_restores_cursor() {
    for terminal in [false, true] {
        let buffer = Buffer::default();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut monitor = FamilyCloseProgressMonitor::with_test_settings(
                buffer.clone(),
                terminal,
                true,
                true,
                Duration::from_millis(10),
                Some(160),
            );
            monitor.observe(generating(4, 7, started_row()));
            panic!("synthetic solver unwind");
        }));
        assert!(caught.is_err());
        let text = buffer.text();
        if terminal {
            // Ratatui emits cell diffs and cursor moves, not contiguous prose.
            // Both outputs use the same Display header; check its exact unwind
            // status through plain output, and the real TTY cleanup separately.
            assert!(text.contains("\u{1b}[?25h"), "{text:?}");
            assert!(text.contains("\u{1b}[0m"), "{text:?}");
            assert!(text.ends_with("\u{1b}[1E"), "{text:?}");
        } else {
            assert!(text.contains("stopped (output unconfirmed)"), "{text:?}");
            assert!(!text.contains("output written"), "{text:?}");
            assert!(!text.contains('\u{1b}') && !text.contains('\r'));
        }
    }
}

struct FailingWriter;
impl Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("expected writer failure"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("expected flush failure"))
    }
}

#[test]
fn failed_presenter_does_not_fail_solver_or_shutdown() {
    for terminal in [false, true] {
        let mut monitor = FamilyCloseProgressMonitor::with_test_settings(
            FailingWriter,
            terminal,
            true,
            true,
            Duration::from_millis(1),
            Some(80),
        );
        monitor.observe(generating(4, 7, started_row()));
        monitor.finish(false);
        monitor.finish(true);
    }
}

#[test]
fn all_final_statuses_remain_output_statuses_not_closure_claims() {
    let now = Instant::now();
    for (outcome, expected) in [
        (Outcome::Written, "output written"),
        (Outcome::Failed, "failed (see error)"),
        (Outcome::Stopped, "stopped (output unconfirmed)"),
    ] {
        let display = Display::new(None, Resources::default(), now, now, Some(outcome));
        assert!(display.header.contains(expected));
        assert!(!display.header.contains("closed"));
        assert!(!display.header.contains("certified"));
    }
}
