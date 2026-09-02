use std::io::{self, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::model::{CampaignPhase, DashboardState, WaveDashboardState, defensible_cap_eta};
use super::render::{render_dashboard, render_dashboard_for_width};
use super::{CampaignProgressMonitor, ColorPolicy, ProgressPresentation, REFRESH_INTERVAL};

fn example_state() -> DashboardState {
    DashboardState {
        phase: CampaignPhase::Discovering,
        elapsed: Duration::from_secs(3_723),
        revision: 42,
        owner_count: 42,
        task_report_ceiling: Some(4_096),
        uncovered_box_count: Some(317),
        effective_dimension: Some(4),
        maximum_dimension: Some(6),
        strict_shrink: 42,
        no_proposal: 2,
        duplicate: 1,
        errors: 0,
        task_reports: 45,
        owner_rate: Some(3.5),
        cap_eta: Some(Duration::from_secs(1_158)),
        rss_bytes: Some(512 * 1_024 * 1_024),
        wave: None,
    }
}

fn enabled_presentation() -> ProgressPresentation {
    ProgressPresentation::resolve(false, ColorPolicy::Never, true, false)
}

#[derive(Clone, Default)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl SharedWriter {
    fn bytes(&self) -> Vec<u8> {
        self.0.lock().unwrap().clone()
    }

    fn len(&self) -> usize {
        self.0.lock().unwrap().len()
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

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed TTY"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed TTY"))
    }
}

#[test]
fn non_tty_is_quiet_and_no_progress_wins() {
    assert_eq!(
        ProgressPresentation::resolve(false, ColorPolicy::Auto, false, false),
        ProgressPresentation {
            enabled: false,
            color: false,
        }
    );
    assert_eq!(
        ProgressPresentation::resolve(false, ColorPolicy::Always, false, false),
        ProgressPresentation {
            enabled: false,
            color: true,
        }
    );
    assert_eq!(
        ProgressPresentation::resolve(true, ColorPolicy::Always, true, false),
        ProgressPresentation {
            enabled: false,
            color: true,
        }
    );
}

#[test]
fn auto_color_honors_tty_and_no_color() {
    assert_eq!(
        ProgressPresentation::resolve(false, ColorPolicy::Auto, true, false),
        ProgressPresentation {
            enabled: true,
            color: true,
        }
    );
    assert_eq!(
        ProgressPresentation::resolve(false, ColorPolicy::Auto, true, true),
        ProgressPresentation {
            enabled: true,
            color: false,
        }
    );
    assert_eq!(
        ProgressPresentation::resolve(false, ColorPolicy::Never, true, false),
        ProgressPresentation {
            enabled: true,
            color: false,
        }
    );
}

#[test]
fn full_dashboard_is_deterministic_stable_width_and_unit_correct() {
    let first = render_dashboard(&example_state());
    let second = render_dashboard(&example_state());
    assert_eq!(first, second);
    assert!(!first.contains("\x1b["), "{first:?}");
    assert!(first.contains("│ phase / stop    │ discovering"), "{first}");
    assert!(first.contains("│ owner rate      │"), "{first}");
    assert!(first.contains("3.50 owners/s"), "{first}");
    assert!(first.contains("│ cap ETA        │"), "{first}");
    assert!(first.contains("00:19:18"), "{first}");
    assert!(
        first.contains("│ owners         │                 42"),
        "{first}"
    );
    assert!(first.contains("45 / 4096"), "{first}");
    assert!(!first.contains("42 / 4096"), "{first}");
    let line_widths = first
        .lines()
        .map(|line| line.chars().count())
        .collect::<Vec<_>>();
    assert!(
        line_widths.windows(2).all(|pair| pair[0] == pair[1]),
        "dashboard lines were not stable-width: {line_widths:?}\n{first}"
    );
}

#[test]
fn cap_eta_requires_one_to_one_progress() {
    assert_eq!(
        defensible_cap_eta(8, 10, true, Some(2.0)),
        Some(Duration::from_secs(1))
    );
    assert_eq!(defensible_cap_eta(8, 10, false, Some(2.0)), None);
    assert_eq!(defensible_cap_eta(8, 10, true, None), None);
}

#[test]
fn narrow_view_uses_compact_clipped_content() {
    let compact = render_dashboard_for_width(&example_state(), 32);
    assert!(compact.starts_with("RustRed · discovering"), "{compact}");
    assert!(compact.contains("reports 45/4096"), "{compact}");
    assert!(!compact.contains("uncovered boxes"), "{compact}");

    let writer = SharedWriter::default();
    let observed = writer.clone();
    let mut monitor = CampaignProgressMonitor::with_test_viewport(
        writer,
        enabled_presentation(),
        Duration::from_millis(10),
        32,
    );
    monitor.start();
    monitor.finish_failed();
    let bytes = observed.bytes();
    assert!(!bytes.windows(4).any(|value| value == b"\x1b[1A"));
    assert!(String::from_utf8_lossy(&bytes).contains("RustRed"));
}

#[test]
fn wave_dashboard_reuses_the_tty_table_without_claiming_single_sector_census() {
    let mut state = example_state();
    state.task_report_ceiling = None;
    state.strict_shrink = 0;
    state.no_proposal = 0;
    state.duplicate = 0;
    state.errors = 0;
    state.wave = Some(WaveDashboardState {
        ordinal: 1,
        active_count: 4,
        orbit_count: 10,
        closed_orbit_count: 3,
        running_orbit_count: 5,
        terminal_orbit_count: 1,
    });

    let full = render_dashboard(&state);
    assert!(full.contains("wave / rank"), "{full}");
    assert!(full.contains("orbits c / r / t"), "{full}");
    assert!(full.contains("2 / 4"), "{full}");
    assert!(full.contains("3 / 5 / 1"), "{full}");
    assert!(!full.contains("strict shrink"), "{full}");
    assert!(!full.contains("\x1b["), "{full:?}");

    let compact = render_dashboard_for_width(&state, 32);
    assert!(compact.contains("wave 2 · rank 4"), "{compact}");
    assert!(compact.contains("orbits 3/10"), "{compact}");
    assert!(compact.contains("running 5 · terminal 1"), "{compact}");
}

#[test]
fn presenter_ticks_without_semantic_callbacks_and_stays_below_configured_rate() {
    assert!(REFRESH_INTERVAL >= Duration::from_millis(100));
    let writer = SharedWriter::default();
    let observed = writer.clone();
    let refresh = Duration::from_millis(20);
    let mut monitor = CampaignProgressMonitor::with_test_refresh_interval(
        writer,
        enabled_presentation(),
        refresh,
    );
    monitor.start();
    wait_until(Duration::from_secs(1), || observed.len() != 0);
    let first = observed.len();
    thread::sleep(refresh + refresh + Duration::from_millis(10));
    let after_ticks = observed.len();
    monitor.finish_failed();
    assert!(
        after_ticks > first,
        "presenter did not tick across a callback gap"
    );
}

#[test]
fn writer_failure_is_nonsemantic_and_fail_open() {
    let mut monitor = CampaignProgressMonitor::with_test_refresh_interval(
        FailingWriter,
        enabled_presentation(),
        Duration::from_millis(5),
    );
    monitor.start();
    thread::sleep(Duration::from_millis(15));
    // Neither a failed first frame nor cleanup may escape into the campaign.
    monitor.finish_failed();
}

#[test]
fn drop_and_unwind_restore_cursor_color_and_line_position() {
    let writer = SharedWriter::default();
    let observed = writer.clone();
    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let mut monitor = CampaignProgressMonitor::with_test_refresh_interval(
            writer,
            enabled_presentation(),
            Duration::from_millis(10),
        );
        monitor.start();
        wait_until(Duration::from_secs(1), || observed.len() != 0);
        panic!("synthetic campaign panic");
    }));
    assert!(panic_result.is_err());
    let bytes = observed.bytes();
    assert!(bytes.windows(6).any(|value| value == b"\x1b[?25h"));
    assert!(bytes.windows(4).any(|value| value == b"\x1b[0m"));
    assert!(bytes.ends_with(b"\x1b[1E"), "cleanup suffix: {bytes:?}");
}

fn wait_until(timeout: Duration, predicate: impl Fn() -> bool) {
    let deadline = Instant::now() + timeout;
    while !predicate() {
        assert!(Instant::now() < deadline, "timed out waiting for presenter");
        thread::sleep(Duration::from_millis(2));
    }
}
