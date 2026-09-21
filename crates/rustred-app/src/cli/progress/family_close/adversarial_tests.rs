//! Independent checks of non-authoritative, bounded progress presentation.

use std::io::{self, Write};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::{FamilyCloseGenerationStage as Stage, FamilyCloseProgress as Event};

use super::format::format_event;
use super::presenter::{Display, FamilyCloseProgressMonitor};
use super::resources::Resources;
use super::state::{MAX_TRACKED_JOBS, Tracker};

fn generation(ordinal: usize, sector: u64, stage: Stage) -> Event {
    Event::Generating {
        ordinal,
        sector,
        stage,
        elapsed: Duration::from_secs(3),
    }
}

#[test]
fn aggregate_counters_survive_every_intermediate_snapshot_being_discarded() {
    let mut tracker = Tracker::default();
    let now = Instant::now();
    tracker.observe(
        Event::Prepared {
            sectors: 10,
            zero_sectors: 0,
            global_zero_sectors: 0,
            elapsed: Duration::ZERO,
        },
        now,
    );
    tracker.observe(
        Event::CheckpointPrepared {
            reused_sectors: 2,
            pending_sectors: 8,
            elapsed: Duration::ZERO,
        },
        now,
    );
    for ordinal in 0..8 {
        tracker.observe(
            Event::GeneratedSector {
                ordinal,
                sector: ordinal as u64,
                rules: ordinal + 1,
                finite_residuals: 2,
                elapsed: Duration::ZERO,
            },
            now,
        );
        tracker.observe(
            Event::CheckpointedSector {
                ordinal,
                sector: ordinal as u64,
                bytes: 100,
                elapsed: Duration::ZERO,
            },
            now,
        );
    }
    let final_snapshot = tracker.observe(
        Event::Encoding {
            elapsed: Duration::ZERO,
        },
        now,
    );
    assert_eq!(final_snapshot.counts.generated, 8);
    assert_eq!(final_snapshot.counts.reused, 2);
    assert_eq!(final_snapshot.counts.checkpointed, 8);
    assert_eq!(final_snapshot.counts.rules, 36);
    assert_eq!(final_snapshot.counts.residuals, 16);
    assert_eq!(final_snapshot.counts.fraction(), Some((10, 10)));
}

#[test]
fn presentation_job_capacity_is_bounded_and_finished_jobs_release_it() {
    let mut tracker = Tracker::default();
    let now = Instant::now();
    for ordinal in 0..MAX_TRACKED_JOBS + 17 {
        tracker.observe(generation(ordinal, 1, Stage::Case { pending: 0 }), now);
    }
    assert_eq!(tracker.jobs.len(), MAX_TRACKED_JOBS);
    let untracked = tracker.observe(
        generation(
            MAX_TRACKED_JOBS + 1,
            1,
            Stage::ExactRowStarted {
                row: 1,
                input_nonzeros: 3,
                reducer_rows: 0,
                reducer_nonzeros: 0,
            },
        ),
        now,
    );
    assert!(untracked.case.is_none());
    assert!(untracked.frame.is_none());
    tracker.observe(
        Event::FailedSector {
            ordinal: 0,
            sector: 1,
            message: "expected test failure".to_owned(),
            elapsed: Duration::ZERO,
        },
        now,
    );
    let tracked = tracker.observe(
        generation(MAX_TRACKED_JOBS + 1, 1, Stage::Case { pending: 0 }),
        now,
    );
    assert_eq!(tracked.case, Some(1));
    assert_eq!(tracker.jobs.len(), MAX_TRACKED_JOBS);
    assert_eq!(tracked.counts.failures, 1);
}

#[test]
fn reused_sector_mask_in_another_job_never_inherits_its_frame() {
    let mut tracker = Tracker::default();
    let now = Instant::now();
    let first = tracker.observe(
        generation(
            0,
            13,
            Stage::ExactFramePrepared {
                source_rows: 100,
                integral_columns: 200,
                target_column: 50,
                input_terms: 1_000,
                coefficient_variables: 6,
                active_variables: 4,
            },
        ),
        now,
    );
    assert_eq!(first.frame.unwrap().source_rows, 100);
    let different_job = tracker.observe(generation(1, 13, Stage::GuardExtraction), now);
    assert!(different_job.frame.is_none());
    let same_job_new_case = tracker.observe(generation(0, 13, Stage::Case { pending: 2 }), now);
    assert!(same_job_new_case.frame.is_none());
    let stale_row = tracker.observe(
        generation(
            0,
            13,
            Stage::ExactRowFinished {
                row: 2,
                accepted_pivot: true,
                reducer_rows: 2,
                reducer_nonzeros: 8,
            },
        ),
        now,
    );
    assert!(stale_row.frame.is_none());
    let rendered = format_event(stale_row.event, stale_row.frame);
    assert!(rendered.contains("frame=unknown"));
    assert!(!rendered.contains("source_rows=100"));
}

#[test]
fn failure_retains_only_bounded_utf8_text_and_bounded_string_capacity() {
    let now = Instant::now();
    for message in ["é\x1b[31m\n".repeat(50_000), {
        let mut short_with_large_capacity = String::with_capacity(1_000_000);
        short_with_large_capacity.push_str("short\rerror");
        short_with_large_capacity
    }] {
        let mut tracker = Tracker::default();
        let snapshot = tracker.observe(
            Event::FailedSector {
                ordinal: 0,
                sector: 9,
                message,
                elapsed: Duration::ZERO,
            },
            now,
        );
        let Event::FailedSector { message, .. } = &snapshot.event else {
            panic!("failure event was not retained");
        };
        assert!(message.len() <= 1_027);
        assert!(
            message.capacity() <= 2_054,
            "retained capacity={}",
            message.capacity()
        );
        let failure = snapshot.last_failure.as_ref().unwrap();
        assert_eq!((failure.ordinal, failure.sector), (0, 9));
        assert!(failure.message.len() <= 1_027);
        assert!(failure.message.capacity() <= 2_054);
        let rendered = format_event(snapshot.event, snapshot.frame);
        assert!(!rendered.contains('\x1b'));
        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\r'));
    }
}

#[test]
fn failure_identity_survives_coalescing_with_another_workers_event() {
    let now = Instant::now();
    let mut tracker = Tracker::default();
    tracker.observe(
        Event::FailedSector {
            ordinal: 17,
            sector: 14343,
            message: "first failure".into(),
            elapsed: Duration::ZERO,
        },
        now,
    );
    tracker.observe(
        Event::FailedSector {
            ordinal: 28,
            sector: 27639,
            message: "é\x1b[31m\n\r".repeat(50_000),
            elapsed: Duration::ZERO,
        },
        now,
    );
    let snapshot = tracker.observe(generation(99, 123, Stage::GuardExtraction), now);
    assert_eq!(snapshot.counts.failures, 2);
    let failure = snapshot.last_failure.as_ref().unwrap();
    assert_eq!((failure.ordinal, failure.sector), (28, 27639));
    assert!(failure.message.len() <= 1_027);
    assert!(failure.message.capacity() <= 2_054);
    let display = Display::new(Some(&snapshot), Resources::default(), now, now, None);
    assert!(display.detail.contains("sector=123"));
    assert!(
        display
            .footer
            .contains("last failure: sector=27639 ordinal=28 message=")
    );
    assert!(!display.footer.contains("sector=123"));
    assert!(!display.footer.contains("first failure"));
    assert!(display.footer.contains('é'));
    for control in ['\x1b', '\n', '\r'] {
        assert!(!display.footer.contains(control));
    }
}

#[test]
fn overflowing_or_inconsistent_counts_never_create_a_completion_fraction() {
    let mut tracker = Tracker::default();
    let now = Instant::now();
    assert!(tracker.counts.fraction().is_none());
    tracker.observe(
        Event::Prepared {
            sectors: 1,
            zero_sectors: 0,
            global_zero_sectors: 0,
            elapsed: Duration::ZERO,
        },
        now,
    );
    for ordinal in 0..2 {
        tracker.observe(
            Event::GeneratedSector {
                ordinal,
                sector: ordinal as u64,
                rules: usize::MAX,
                finite_residuals: 0,
                elapsed: Duration::ZERO,
            },
            now,
        );
    }
    assert!(tracker.counts.fraction().is_none());
}

#[test]
fn process_memory_parser_rejects_wrong_units_and_overflow_as_unknown() {
    let valid = Resources::parse("Name:\trustred\nVmRSS:\t1024 kB\nVmHWM:\t2048 kB\n");
    assert_eq!(valid.rss, Some(1_048_576));
    assert_eq!(valid.peak_rss, Some(2_097_152));
    let rendered = valid.line();
    assert!(rendered.contains("process RSS=1.0 MiB"));
    assert!(rendered.contains("peak=2.0 MiB"));
    assert!(rendered.contains("not process-tree memory"));
    for status in [
        "VmRSS: 10 MB\nVmHWM: 20 bytes\n",
        "VmRSS: -1 kB\nVmHWM: unknown kB\n",
        "VmRSS: 18446744073709551615 kB\n",
        "VmRSS:other 10 kB\nVmHWM:other 20 kB\n",
        "",
    ] {
        let resources = Resources::parse(status);
        assert!(resources.rss.is_none());
        assert!(resources.peak_rss.is_none());
        assert!(resources.line().contains("unavailable"));
    }
}

#[test]
fn phase_age_survives_row_updates_but_not_a_switch_to_another_job() {
    let mut tracker = Tracker::default();
    let start = Instant::now();
    let row = |ordinal, number| {
        generation(
            ordinal,
            7,
            Stage::ExactRowStarted {
                row: number,
                input_nonzeros: 4,
                reducer_rows: 3,
                reducer_nonzeros: 9,
            },
        )
    };
    let first = tracker.observe(row(0, 1), start);
    let next_row = tracker.observe(row(0, 2), start + Duration::from_secs(4));
    assert_eq!(first.phase_since, start);
    assert_eq!(next_row.phase_since, start);
    let another_job = tracker.observe(row(1, 3), start + Duration::from_secs(5));
    assert_eq!(another_job.phase_since, start + Duration::from_secs(5));
}

#[derive(Clone, Default)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl SharedWriter {
    fn bytes(&self) -> Vec<u8> {
        self.0.lock().unwrap().clone()
    }
}

impl Write for SharedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn default_non_terminal_monitor_remains_silent_even_on_failure_and_drop() {
    let writer = SharedWriter::default();
    let observed = writer.clone();
    let mut monitor = FamilyCloseProgressMonitor::with_test_settings(
        writer,
        false,
        false,
        false,
        Duration::from_millis(5),
        None,
    );
    monitor.observe(Event::FailedSector {
        ordinal: 0,
        sector: 2,
        message: "failure must not enable default pipe output".into(),
        elapsed: Duration::ZERO,
    });
    monitor.finish(false);
    drop(monitor);
    assert!(observed.bytes().is_empty());
}

#[test]
fn forced_plain_output_heartbeats_without_callbacks_and_never_emits_escapes() {
    let writer = SharedWriter::default();
    let observed = writer.clone();
    let mut monitor = FamilyCloseProgressMonitor::with_test_settings(
        writer,
        false,
        true,
        false,
        Duration::from_millis(10),
        None,
    );
    monitor.observe(Event::Preparing {
        arity: 7,
        elapsed: Duration::from_secs(4),
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while observed
        .bytes()
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        < 3
        && Instant::now() < deadline
    {
        thread::sleep(Duration::from_millis(5));
    }
    let heartbeat_lines = observed
        .bytes()
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count();
    monitor.finish(true);
    let before_drop = observed.bytes();
    monitor.finish(false);
    drop(monitor);
    assert_eq!(
        observed.bytes(),
        before_drop,
        "finish/drop must be idempotent"
    );
    assert!(heartbeat_lines >= 3, "no heartbeat across callback silence");
    let text = String::from_utf8(before_drop).unwrap();
    assert!(!text.contains('\x1b'));
    assert!(!text.contains('\r'));
    assert!(text.contains("quiet="));
    assert!(text.contains("last event:"));
    assert!(text.contains("output written"));
    assert!(text.contains("total unknown (no ETA)"));
    assert!(!text.contains("family closed"));
}

struct BlockingWriter {
    gate: Arc<(Mutex<bool>, Condvar)>,
    entered: Option<mpsc::Sender<()>>,
}

impl Write for BlockingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(entered) = self.entered.take() {
            let _ = entered.send(());
        }
        let (lock, wake) = &*self.gate;
        let mut released = lock.lock().unwrap();
        while !*released {
            released = wake.wait(released).unwrap();
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn unblock(gate: &Arc<(Mutex<bool>, Condvar)>) {
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
}

#[test]
fn solver_callback_does_not_wait_for_a_blocked_terminal_writer() {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (entered, at_writer) = mpsc::channel();
    let mut monitor = FamilyCloseProgressMonitor::with_test_settings(
        BlockingWriter {
            gate: gate.clone(),
            entered: Some(entered),
        },
        false,
        true,
        true,
        Duration::from_millis(10),
        None,
    );
    let writer_is_blocked = at_writer.recv_timeout(Duration::from_secs(2));
    let (sent, returned) = mpsc::channel();
    let worker = thread::spawn(move || {
        monitor.observe(Event::GeneratedSector {
            ordinal: 0,
            sector: 3,
            rules: 9,
            finite_residuals: 2,
            elapsed: Duration::from_secs(1),
        });
        let _ = sent.send(());
        monitor
    });
    let callback_returned_while_blocked = returned.recv_timeout(Duration::from_secs(2));
    // Release before every assertion/join: a regression must fail, not strand
    // a test presenter inside an intentionally blocked writer.
    unblock(&gate);
    let mut monitor = worker.join().unwrap();
    monitor.finish(true);
    assert!(writer_is_blocked.is_ok());
    assert!(callback_returned_while_blocked.is_ok());
}

#[test]
fn shutdown_has_a_bounded_grace_when_the_writer_never_returns() {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (entered, at_writer) = mpsc::channel();
    let monitor = FamilyCloseProgressMonitor::with_test_settings(
        BlockingWriter {
            gate: gate.clone(),
            entered: Some(entered),
        },
        false,
        true,
        true,
        Duration::from_millis(10),
        None,
    );
    let writer_is_blocked = at_writer.recv_timeout(Duration::from_secs(2));
    let (sent, returned) = mpsc::channel();
    let worker = thread::spawn(move || {
        drop(monitor);
        let _ = sent.send(());
    });
    let dropped_while_blocked = returned.recv_timeout(Duration::from_secs(2));
    unblock(&gate);
    worker.join().unwrap();
    assert!(writer_is_blocked.is_ok());
    assert!(
        dropped_while_blocked.is_ok(),
        "blocked writer prevented bounded shutdown"
    );
}
