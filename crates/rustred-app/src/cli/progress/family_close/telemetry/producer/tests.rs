use super::*;
use crate::FamilyCloseGenerationStage as G;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let suffix = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rustred-generation-progress-{}-{stamp}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("progress.json")
    }
    fn read(&self) -> NativeSnapshot {
        serde_json::from_slice(&std::fs::read(self.path()).unwrap()).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn headless_export_counts_all_events_before_coalescing_and_finishes_after_output() {
    let directory = Directory::new();
    let mut monitor = FamilyGenerationTelemetry::new(Some(directory.path()));
    monitor.observe(FamilyCloseProgress::Prepared {
        sectors: 100,
        zero_sectors: 0,
        global_zero_sectors: 0,
        elapsed: Duration::ZERO,
    });
    monitor.observe(FamilyCloseProgress::CheckpointPrepared {
        reused_sectors: 20,
        pending_sectors: 80,
        elapsed: Duration::ZERO,
    });
    for ordinal in 0..80 {
        monitor.observe(FamilyCloseProgress::GeneratedSector {
            ordinal,
            sector: ordinal as u64,
            rules: 2,
            finite_residuals: 3,
            elapsed: Duration::ZERO,
        });
        monitor.observe(FamilyCloseProgress::CheckpointedSector {
            ordinal,
            sector: ordinal as u64,
            bytes: 100,
            elapsed: Duration::ZERO,
        });
    }
    monitor.observe(FamilyCloseProgress::Encoded {
        bytes: 123,
        elapsed: Duration::ZERO,
    });
    // Encoded alone cannot claim the CLI write succeeded.
    assert!(
        monitor
            .shared
            .as_ref()
            .unwrap()
            .state
            .lock()
            .unwrap()
            .outcome
            .is_none()
    );
    monitor.finish(true);
    let snapshot = directory.read();
    snapshot.validate().unwrap();
    assert_eq!(snapshot.state, NativeState::OutputWritten);
    assert_eq!(snapshot.counts.generated, 80);
    assert_eq!(snapshot.counts.reused, 20);
    assert_eq!(snapshot.counts.checkpointed_new, 80);
    assert_eq!(snapshot.counts.rules_generated, 160);
    assert_eq!(snapshot.counts.finite_residuals_generated, 240);
    assert!(!snapshot.family_closure_claim);
    assert!(!snapshot.checkpoint.in_sector_resume);
    let final_bytes = std::fs::read(directory.path()).unwrap();
    monitor.finish(false);
    drop(monitor);
    assert_eq!(std::fs::read(directory.path()).unwrap(), final_bytes);
}

#[test]
fn export_keeps_bounded_concurrent_job_detail_with_independent_phase_ages() {
    let directory = Directory::new();
    let mut monitor = FamilyGenerationTelemetry::new(Some(directory.path()));
    for ordinal in 0..300 {
        monitor.observe(FamilyCloseProgress::Generating {
            ordinal,
            sector: 1,
            stage: G::Case { pending: 4 },
            elapsed: Duration::ZERO,
        });
    }
    monitor.observe(FamilyCloseProgress::Generating {
        ordinal: 0,
        sector: 1,
        stage: G::ExactFramePrepared {
            source_rows: 9,
            integral_columns: 13,
            target_column: 2,
            input_terms: 21,
            coefficient_variables: 3,
            active_variables: 2,
        },
        elapsed: Duration::ZERO,
    });
    monitor.finish(false);
    let snapshot = directory.read();
    snapshot.validate().unwrap();
    assert_eq!(snapshot.active_jobs.len(), 256);
    assert!(snapshot.details_truncated);
    assert_eq!(
        snapshot.active_jobs[0].frame.as_ref().unwrap().source_rows,
        9
    );
    assert_eq!(snapshot.active_jobs[1].phase, "case selection");
    assert_eq!(snapshot.active_jobs[0].phase, "exact frame");
    assert_eq!(snapshot.state, NativeState::Failed);
}

#[test]
fn failed_publication_is_nonsemantic_and_disabled_monitor_is_inert() {
    let directory = Directory::new();
    let mut monitor =
        FamilyGenerationTelemetry::new(Some(directory.0.join("missing/progress.json")));
    monitor.observe(FamilyCloseProgress::Encoded {
        bytes: 100,
        elapsed: Duration::ZERO,
    });
    monitor.finish(true);
    assert!(!directory.0.join("missing").exists());
    let mut disabled = FamilyGenerationTelemetry::new(None);
    disabled.observe(FamilyCloseProgress::Encoded {
        bytes: 100,
        elapsed: Duration::ZERO,
    });
    disabled.finish(true);
    assert!(disabled.shared.is_none());
}
