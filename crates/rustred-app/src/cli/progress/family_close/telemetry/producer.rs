//! Independent JSON publisher. Human terminal I/O cannot delay this worker.

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::super::state::{ExactJob, Snapshot, Tracker};
use super::{
    ActiveJob, CheckpointSemantics, FrameSnapshot, NativeCounts, NativeFailure, NativeSnapshot,
    NativeState, SCHEMA, process_start_ticks,
};
use crate::FamilyCloseProgress;

const INTERVAL: Duration = Duration::from_secs(1);
const SHUTDOWN_GRACE: Duration = Duration::from_millis(500);

#[derive(Default)]
struct State {
    tracker: Tracker,
    latest: Option<Snapshot>,
    revision: u64,
    outcome: Option<NativeState>,
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

/// Optional presentation only. No I/O or DTO formatting in `observe`.
pub(crate) struct FamilyGenerationTelemetry {
    shared: Option<Arc<Shared>>,
    thread: Option<JoinHandle<()>>,
    completed: Option<mpsc::Receiver<()>>,
}

impl FamilyGenerationTelemetry {
    pub(crate) fn new(path: Option<PathBuf>) -> Self {
        let mut monitor = Self {
            shared: None,
            thread: None,
            completed: None,
        };
        let Some(path) = path else {
            return monitor;
        };
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
        });
        let state = shared.clone();
        let (sent, received) = mpsc::channel();
        let started = Instant::now();
        let pid = std::process::id();
        let ticks = process_start_ticks();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let invocation_id = format!(
            "{pid}-{}-{stamp}",
            ticks.map_or_else(|| "unknown".into(), |n| n.to_string())
        );
        let spawned = thread::Builder::new()
            .name("rustred-generation-json".into())
            .spawn(move || {
                let _completion = Completion(sent);
                publish(path, started, pid, ticks, invocation_id, &state);
            });
        if let Ok(thread) = spawned {
            monitor.shared = Some(shared);
            monitor.thread = Some(thread);
            monitor.completed = Some(received);
        }
        monitor
    }

    pub(crate) fn observe(&mut self, event: FamilyCloseProgress) {
        if let Some(shared) = &self.shared {
            if let Ok(mut state) = shared.state.lock() {
                if state.outcome.is_none() {
                    state.revision = state.revision.saturating_add(1);
                    state.latest = Some(state.tracker.observe(event, Instant::now()));
                }
            }
        }
    }

    pub(crate) fn finish(&mut self, success: bool) {
        self.stop(if success {
            NativeState::OutputWritten
        } else {
            NativeState::Failed
        });
    }

    fn stop(&mut self, outcome: NativeState) {
        let Some(shared) = self.shared.take() else {
            return;
        };
        if let Ok(mut state) = shared.state.lock() {
            state.outcome.get_or_insert(outcome);
            state.revision = state.revision.saturating_add(1);
        }
        shared.wake.notify_all();
        // A blocked filesystem never hangs the solver on shutdown. This thread
        // owns only its latest-snapshot path, never checkpoint/output authority.
        let completed = self.completed.take().is_some_and(|done| {
            !matches!(
                done.recv_timeout(SHUTDOWN_GRACE),
                Err(mpsc::RecvTimeoutError::Timeout)
            )
        });
        if let Some(thread) = self.thread.take() {
            if completed {
                let _ = thread.join();
            }
        }
    }
}

impl Drop for FamilyGenerationTelemetry {
    fn drop(&mut self) {
        self.stop(NativeState::Stopped);
    }
}
struct Completion(mpsc::Sender<()>);
impl Drop for Completion {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn publish(
    path: PathBuf,
    started: Instant,
    pid: u32,
    process_start_ticks: Option<u64>,
    invocation_id: String,
    shared: &Shared,
) {
    loop {
        let (latest, jobs, details_truncated, revision, outcome) = {
            let Ok(state) = shared.state.lock() else {
                return;
            };
            // O(256) scalar copies only once per publication, not per callback.
            (
                state.latest.clone(),
                state.tracker.jobs.clone(),
                state.tracker.details_truncated,
                state.revision,
                state.outcome,
            )
        };
        let now = Instant::now();
        let counts = latest
            .as_ref()
            .map(|snapshot| snapshot.counts)
            .unwrap_or_default();
        let native = NativeSnapshot {
            schema: SCHEMA.into(),
            pid,
            process_start_ticks,
            invocation_id: invocation_id.clone(),
            revision,
            state: outcome.unwrap_or(NativeState::Running),
            elapsed_seconds: now.duration_since(started).as_secs_f64(),
            last_event_elapsed_seconds: latest
                .as_ref()
                .map(|snapshot| snapshot.at.saturating_duration_since(started).as_secs_f64()),
            counts: NativeCounts {
                sectors_total: counts.total.map(|n| n as u64),
                generated: counts.generated as u64,
                reused: counts.reused as u64,
                checkpointed_new: counts.checkpointed as u64,
                rules_generated: counts.rules as u64,
                finite_residuals_generated: counts.residuals as u64,
                observed_rule_hits: counts.observed_rules as u64,
                failures: counts.failures as u64,
                overflow: counts.overflow,
            },
            active_jobs: jobs
                .into_iter()
                .filter_map(|(key, job)| active_job(key, job, started, now))
                .collect(),
            details_truncated,
            last_failure: latest
                .and_then(|snapshot| snapshot.last_failure)
                .map(|failure| NativeFailure {
                    ordinal: failure.ordinal as u64,
                    sector_mask: failure.sector.to_string(),
                    message: failure.message,
                }),
            checkpoint: CheckpointSemantics::default(),
            family_closure_claim: false,
        };
        // Publication failure is diagnostic-only and may recover on the next
        // heartbeat; neither a failure nor a final read changes solver results.
        let _ = native.publish(&path);
        if outcome.is_some() {
            return;
        }
        let deadline = Instant::now() + INTERVAL;
        let Ok(mut state) = shared.state.lock() else {
            return;
        };
        while state.outcome.is_none() {
            let wait = deadline.saturating_duration_since(Instant::now());
            if wait.is_zero() {
                break;
            }
            let Ok((next, _)) = shared.wake.wait_timeout(state, wait) else {
                return;
            };
            state = next;
        }
    }
}

fn active_job(
    (ordinal, sector): (usize, u64),
    job: ExactJob,
    started: Instant,
    now: Instant,
) -> Option<ActiveJob> {
    Some(ActiveJob {
        ordinal: ordinal as u64,
        sector_mask: sector.to_string(),
        phase: job.phase?.into(),
        phase_elapsed_seconds: now
            .saturating_duration_since(job.phase_since?)
            .as_secs_f64(),
        last_event_elapsed_seconds: job.at?.saturating_duration_since(started).as_secs_f64(),
        case_index: job.case.filter(|n| *n != 0).map(|n| n as u64),
        frame: job.frame.map(|frame| FrameSnapshot {
            index: frame.sequence.map(|n| n as u64),
            source_rows: frame.source_rows as u64,
            integral_columns: frame.integral_columns as u64,
            target_column: frame.target_column as u64,
            input_terms: frame.input_terms as u64,
            coefficient_variables: frame.coefficient_variables as u64,
            active_variables: frame.active_variables as u64,
        }),
        detail: job.stage?.into(),
    })
}

#[cfg(test)]
mod tests;
