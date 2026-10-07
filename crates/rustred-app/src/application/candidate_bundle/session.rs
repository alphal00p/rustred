//! In-process, bounded observations over the ordinary candidate generator.
//! A host schedules `CandidateGenerationJob::run`; no Python object or callback
//! enters a worker. Cancellation is cooperative, never a CAS interruption.

use std::collections::VecDeque;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use rustred::family::IntegralFamily;
use serde::Serialize;
use serde_json::{Value, json};

use super::{CandidateBundleResult, FamilyCandidatesRequest, generate};
use crate::application::generation_progress::Tracker;
use crate::{AppError, FamilyCloseProgress};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationState {
    Queued,
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}

impl CandidateGenerationState {
    fn done(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidateGenerationSnapshot {
    pub state: CandidateGenerationState,
    pub done: bool,
    pub elapsed_seconds: f64,
    pub counts: Value,
    pub active_jobs: Vec<Value>,
    pub details_truncated: bool,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidateGenerationEvents {
    pub schema: &'static str,
    pub events: Vec<Value>,
    pub snapshot: CandidateGenerationSnapshot,
    pub dropped_events: u64,
}

struct Inner {
    state: CandidateGenerationState,
    tracker: Tracker,
    events: VecDeque<Value>,
    capacity: usize,
    sequence: u64,
    dropped: u64,
    error: Option<AppError>,
    result: Option<Arc<CandidateBundleResult>>,
    finished: Option<Instant>,
}

struct Shared {
    creator_pid: u32,
    started: Instant,
    cancel: Arc<AtomicBool>,
    inner: Mutex<Inner>,
    changed: Condvar,
}

/// The consumer handle. Dropping it requests stop; the producer retains state
/// until all native workers have drained. No success is inferred from progress.
pub struct CandidateGenerationSession {
    shared: Arc<Shared>,
}

/// A single-use job for a host's stable Symbolica coordinator thread.
pub struct CandidateGenerationJob {
    shared: Arc<Shared>,
    request: Option<FamilyCandidatesRequest>,
    family: Option<Arc<IntegralFamily>>,
}

impl CandidateGenerationSession {
    pub fn prepare(
        request: FamilyCandidatesRequest,
        event_capacity: usize,
    ) -> Result<(Self, CandidateGenerationJob), AppError> {
        Self::prepare_inner(request, None, event_capacity)
    }

    /// Uses the exact native family and parameter identities supplied by a host.
    /// No source serialization, re-parsing, denominator matching or reordering.
    pub fn from_family(
        family: Arc<IntegralFamily>,
        options: FamilyCandidatesRequest,
        event_capacity: usize,
    ) -> Result<(Self, CandidateGenerationJob), AppError> {
        Self::prepare_inner(options, Some(family), event_capacity)
    }

    fn prepare_inner(
        request: FamilyCandidatesRequest,
        family: Option<Arc<IntegralFamily>>,
        capacity: usize,
    ) -> Result<(Self, CandidateGenerationJob), AppError> {
        if capacity == 0 || capacity > 65_536 {
            return Err(AppError::input("event_capacity must be from 1 to 65536"));
        }
        if cfg!(target_arch = "wasm32") && request.n_cores != 1 {
            return Err(AppError::input(
                "WebAssembly candidate generation requires n_cores = 1",
            ));
        }
        let shared = Arc::new(Shared {
            creator_pid: std::process::id(),
            started: Instant::now(),
            cancel: Arc::new(AtomicBool::new(false)),
            inner: Mutex::new(Inner {
                state: CandidateGenerationState::Queued,
                tracker: Tracker::default(),
                events: VecDeque::new(),
                capacity,
                sequence: 0,
                dropped: 0,
                error: None,
                result: None,
                finished: None,
            }),
            changed: Condvar::new(),
        });
        Ok((
            Self {
                shared: shared.clone(),
            },
            CandidateGenerationJob {
                shared,
                request: Some(request),
                family,
            },
        ))
    }

    pub fn check_process(&self) -> Result<(), AppError> {
        if self.shared.creator_pid != std::process::id() {
            return Err(AppError::execution(
                "generation sessions cannot be reused after fork; create a fresh process session",
            ));
        }
        Ok(())
    }
    pub fn cancel(&self) -> Result<(), AppError> {
        self.check_process()?;
        let mut inner = self.shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !inner.state.done() {
            self.shared.cancel.store(true, Ordering::Release);
            inner.state = CandidateGenerationState::Cancelling;
            self.shared.changed.notify_all();
        }
        Ok(())
    }
    pub fn done(&self) -> Result<bool, AppError> {
        self.check_process()?;
        Ok(self
            .shared
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .state
            .done())
    }
    pub fn snapshot(&self) -> Result<CandidateGenerationSnapshot, AppError> {
        self.check_process()?;
        let inner = self.shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        Ok(snapshot(&self.shared, &inner))
    }
    pub fn wait(&self, timeout: Option<Duration>) -> Result<bool, AppError> {
        self.check_process()?;
        let inner = self.shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        // The Pyodide host executes the job inline, not on a background OS
        // thread. Waiting on a condition variable cannot make progress there.
        #[cfg(target_arch = "wasm32")]
        {
            let _ = timeout;
            return Ok(inner.state.done());
        }
        #[cfg(not(target_arch = "wasm32"))]
        Ok(if let Some(timeout) = timeout {
            self.shared
                .changed
                .wait_timeout_while(inner, timeout, |i| !i.state.done())
                .unwrap_or_else(|e| e.into_inner())
                .0
                .state
                .done()
        } else {
            self.shared
                .changed
                .wait_while(inner, |i| !i.state.done())
                .unwrap_or_else(|e| e.into_inner())
                .state
                .done()
        })
    }
    pub fn poll_events(
        &self,
        max_events: usize,
        timeout: Duration,
    ) -> Result<CandidateGenerationEvents, AppError> {
        self.check_process()?;
        if max_events == 0 || max_events > 65_536 {
            return Err(AppError::input("max_events must be from 1 to 65536"));
        }
        let inner = self.shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        #[cfg(not(target_arch = "wasm32"))]
        let mut inner = self
            .shared
            .changed
            .wait_timeout_while(inner, timeout, |i| i.events.is_empty() && !i.state.done())
            .unwrap_or_else(|e| e.into_inner())
            .0;
        #[cfg(target_arch = "wasm32")]
        let mut inner = {
            let _ = timeout;
            inner
        };
        let count = max_events.min(inner.events.len());
        let events = inner.events.drain(..count).collect();
        Ok(CandidateGenerationEvents {
            schema: "rustred.candidate-generation-events.v1",
            events,
            snapshot: snapshot(&self.shared, &inner),
            dropped_events: inner.dropped,
        })
    }
    pub fn result(&self) -> Result<Arc<CandidateBundleResult>, AppError> {
        self.check_process()?;
        let inner = self.shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.result.clone().ok_or_else(|| {
            inner.error.clone().unwrap_or_else(|| {
                AppError::execution(format!(
                    "candidate session is {:?}, not completed",
                    inner.state
                ))
            })
        })
    }
}

impl Drop for CandidateGenerationSession {
    fn drop(&mut self) {
        // A fork child may inherit a permanently locked mutex. Never touch it.
        if self.shared.creator_pid == std::process::id() {
            let _ = self.cancel();
        }
    }
}

impl CandidateGenerationJob {
    pub fn run(mut self) {
        if self.shared.creator_pid != std::process::id() {
            return;
        }
        {
            let mut inner = self.shared.inner.lock().unwrap_or_else(|e| e.into_inner());
            if !self.shared.cancel.load(Ordering::Acquire) {
                inner.state = CandidateGenerationState::Running;
            }
        }
        let request = self.request.take().expect("single-use generation job");
        let observe = |event| observe(&self.shared, event);
        let result = match self.family.take() {
            Some(family) => generate::family_candidates_from_family_controlled(
                family,
                request,
                self.shared.cancel.clone(),
                observe,
            ),
            None => {
                generate::family_candidates_controlled(request, self.shared.cancel.clone(), observe)
            }
        };
        let mut inner = self.shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        match result {
            Ok(result) => {
                inner.result = Some(Arc::new(result));
                inner.state = CandidateGenerationState::Completed;
            }
            Err(error) => {
                // Only the explicit cancellation sentinel is cancellation; a
                // simultaneous native/output failure remains a failed session.
                inner.state = if error.kind() == crate::AppErrorKind::Cancelled
                    && inner.tracker.counts.failures == 0
                {
                    CandidateGenerationState::Cancelled
                } else {
                    CandidateGenerationState::Failed
                };
                if inner.error.is_none() || error.kind() != crate::AppErrorKind::Cancelled {
                    inner.error = Some(error);
                }
            }
        }
        inner.finished = Some(Instant::now());
        self.shared.changed.notify_all();
    }
}

impl Drop for CandidateGenerationJob {
    fn drop(&mut self) {
        if self.shared.creator_pid != std::process::id() {
            return;
        }
        let mut inner = self.shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !inner.state.done() {
            inner.state = CandidateGenerationState::Failed;
            inner.error = Some(AppError::internal_invariant(
                "generation job was not run or panicked; coordinator may be poisoned",
            ));
            inner.finished = Some(Instant::now());
            self.shared.changed.notify_all();
        }
    }
}

fn snapshot(shared: &Shared, inner: &Inner) -> CandidateGenerationSnapshot {
    let c = inner.tracker.counts;
    CandidateGenerationSnapshot {
        state: inner.state,
        done: inner.state.done(),
        elapsed_seconds: inner
            .finished
            .unwrap_or_else(Instant::now)
            .duration_since(shared.started)
            .as_secs_f64(),
        counts: json!({"sectors_total": c.total, "generated": c.generated, "checkpointed": c.checkpointed,
            "reused": c.reused, "rules": c.rules, "finite_residuals": c.residuals,
            "failures": c.failures, "observed_rules": c.observed_rules, "overflow": c.overflow}),
        active_jobs: inner
            .tracker
            .jobs
            .iter()
            .map(|((ordinal, sector), job)| {
                json!({
                    "ordinal": ordinal, "sector": sector, "phase": job.phase, "stage": job.stage,
                    "case": job.case, "sequence": job.sequence,
                })
            })
            .collect(),
        details_truncated: inner.tracker.details_truncated,
        last_error: inner.error.as_ref().map(ToString::to_string),
    }
}

fn observe(shared: &Shared, event: FamilyCloseProgress) {
    let mut inner = shared.inner.lock().unwrap_or_else(|e| e.into_inner());
    let tracked = inner.tracker.observe(event, Instant::now());
    if let Some(failure) = &tracked.last_failure {
        inner.error = Some(AppError::execution(failure.message.clone()));
    }
    let mut event = match tracked.event {
        FamilyCloseProgress::Generating {
            ordinal,
            sector,
            stage,
            ..
        } => json!({"kind":"generating","ordinal":ordinal,"sector":sector,"stage":stage}),
        FamilyCloseProgress::GeneratedSector {
            ordinal,
            sector,
            rules,
            finite_residuals,
            ..
        } => {
            json!({"kind":"generated_sector","ordinal":ordinal,"sector":sector,"rules":rules,"finite_residuals":finite_residuals})
        }
        FamilyCloseProgress::FailedSector {
            ordinal,
            sector,
            message,
            ..
        } => json!({"kind":"failed_sector","ordinal":ordinal,"sector":sector,"message":message}),
        FamilyCloseProgress::CheckpointedSector {
            ordinal,
            sector,
            bytes,
            ..
        } => json!({"kind":"checkpointed_sector","ordinal":ordinal,"sector":sector,"bytes":bytes}),
        FamilyCloseProgress::Prepared {
            sectors,
            zero_sectors,
            global_zero_sectors,
            ..
        } => {
            json!({"kind":"prepared","sectors":sectors,"zero_sectors":zero_sectors,"global_zero_sectors":global_zero_sectors})
        }
        FamilyCloseProgress::CheckpointPrepared {
            reused_sectors,
            pending_sectors,
            ..
        } => {
            json!({"kind":"checkpoint_prepared","reused_sectors":reused_sectors,"pending_sectors":pending_sectors})
        }
        FamilyCloseProgress::Preparing { arity, .. } => json!({"kind":"preparing","arity":arity}),
        FamilyCloseProgress::Encoded { bytes, .. } => json!({"kind":"encoded","bytes":bytes}),
        _ => json!({"kind":tracked.phase}),
    };
    inner.sequence = inner.sequence.saturating_add(1);
    event["sequence"] = json!(inner.sequence);
    event["elapsed_seconds"] = json!(shared.started.elapsed().as_secs_f64());
    if inner.events.len() == inner.capacity {
        inner.events.pop_front();
        inner.dropped = inner.dropped.saturating_add(1);
    }
    inner.events.push_back(event);
    shared.changed.notify_all();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_failed_job_is_observable_without_waiting() {
        let (session, job) =
            CandidateGenerationSession::prepare(FamilyCandidatesRequest::new("unused"), 4).unwrap();
        assert!(!session.wait(Some(Duration::ZERO)).unwrap());
        drop(job);
        assert!(session.wait(Some(Duration::ZERO)).unwrap());
        let events = session.poll_events(4, Duration::ZERO).unwrap();
        assert!(events.snapshot.done);
        assert_eq!(events.snapshot.state, CandidateGenerationState::Failed);
        assert!(session.result().is_err());
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn wasm_sessions_reject_multiple_workers_at_admission() {
        let mut request = FamilyCandidatesRequest::new("unused");
        request.n_cores = 2;
        let error = CandidateGenerationSession::prepare(request, 4)
            .err()
            .unwrap();
        assert_eq!(error.kind(), crate::AppErrorKind::Input);
        assert!(error.to_string().contains("n_cores = 1"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn concurrent_progress_coalesces_events_without_losing_counts() {
        let (session, job) =
            CandidateGenerationSession::prepare(FamilyCandidatesRequest::new("unused"), 4).unwrap();
        std::thread::scope(|scope| {
            for worker in 0..8 {
                let shared = &session.shared;
                scope.spawn(move || {
                    for i in 0..100 {
                        observe(
                            shared,
                            FamilyCloseProgress::GeneratedSector {
                                ordinal: worker * 100 + i,
                                sector: 1,
                                rules: 2,
                                finite_residuals: 3,
                                elapsed: Duration::ZERO,
                            },
                        );
                    }
                });
            }
        });
        let batch = session.poll_events(4, Duration::ZERO).unwrap();
        assert_eq!(batch.events.len(), 4);
        assert_eq!(batch.dropped_events, 796);
        assert_eq!(batch.snapshot.counts["generated"], 800);
        assert_eq!(batch.snapshot.counts["rules"], 1600);
        assert_eq!(batch.snapshot.counts["finite_residuals"], 2400);
        assert!(!batch.snapshot.done);
        drop(job);
    }

    #[test]
    fn dropped_consumer_requests_stop_but_does_not_finish_worker_state() {
        let (session, job) =
            CandidateGenerationSession::prepare(FamilyCandidatesRequest::new("unused"), 4).unwrap();
        let shared = job.shared.clone();
        drop(session);
        assert!(shared.cancel.load(Ordering::Acquire));
        assert_eq!(
            shared.inner.lock().unwrap().state,
            CandidateGenerationState::Cancelling
        );
        job.run();
        assert_eq!(
            shared.inner.lock().unwrap().state,
            CandidateGenerationState::Cancelled
        );
    }
}
