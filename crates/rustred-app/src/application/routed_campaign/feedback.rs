//! Opt-in, retained source-feedback sessions. No automatic trace-only mutation.
mod nomination;
#[cfg(test)]
mod tests;

use std::fmt::{self, Write};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use rustred::family::IntegralKey;
use rustred::solver::{
    CandidateOwnerPrograms, CandidateRoutedCampaignError, CandidateRoutedCampaignFailure,
    CandidateRoutedCampaignReport, CandidateRoutedCampaignSnapshot, FiniteCasePolicy,
    OwnerDomainAttemptLimits, OwnerDomainScope, OwnerFeedbackError, OwnerFeedbackPolicy,
    OwnerOverlayLimits, RoutedCandidateReducer, SectorEvent,
};
use serde_json::{Value, json};

use super::{RoutedCampaignRequest, input, prepare, snapshot_json_with_failure};
use crate::AppError;
use nomination::{Ray, nominate};

/// Explicit policy for NEW source work. It never authenticates historical
/// artifact settings. One native source job runs at a time, between trace pools.
#[derive(Clone, Copy, Debug)]
pub struct RoutedFeedbackOptions {
    pub prospective_policy: OwnerFeedbackPolicy,
    pub finite_case_policy: FiniteCasePolicy,
    pub attempt_limits: OwnerDomainAttemptLimits,
    /// Cumulative installed-overlay limits, independent of raw staging.
    pub overlay_limits: OwnerOverlayLimits,
    pub max_jobs_per_round: usize,
    /// Logical compact nomination storage; excludes allocator overhead.
    pub max_nomination_bytes: usize,
    /// Cumulative installed-ray ledger across every round of this session.
    pub max_installed_jobs: usize,
    /// At most one raw native result is staged before immediate atomic append.
    /// This is not a native search transient/RSS or prepared-output bound.
    pub max_staged_native_bytes: usize,
    /// Per-error bounded Debug payload. Truncation is explicit in the receipt.
    pub max_error_bytes: usize,
}
impl RoutedFeedbackOptions {
    /// Search policy and finite-domain behavior must be supplied deliberately.
    pub fn new(policy: OwnerFeedbackPolicy, finite_case_policy: FiniteCasePolicy) -> Self {
        Self {
            prospective_policy: policy,
            finite_case_policy,
            attempt_limits: Default::default(),
            overlay_limits: Default::default(),
            max_jobs_per_round: 128,
            max_nomination_bytes: 1024 * 1024,
            max_installed_jobs: 1024,
            max_staged_native_bytes: 128 * 1024 * 1024,
            max_error_bytes: 64 * 1024,
        }
    }

    fn validate<const N: usize>(&self) -> Result<(), AppError> {
        if self.max_jobs_per_round == 0
            || self.max_installed_jobs == 0
            || self.max_staged_native_bytes == 0
            || self.max_error_bytes == 0
            || self.max_nomination_bytes < std::mem::size_of::<Ray<N>>()
            || self.attempt_limits.max_requested_cases == 0
        {
            return Err(AppError::input(
                "feedback resource limits must admit at least one job",
            ));
        }
        self.max_installed_jobs
            .checked_mul(std::mem::size_of::<Ray<N>>())
            .ok_or_else(|| AppError::input("feedback ledger byte count overflow"))?;
        if receipt_bound(self.max_jobs_per_round, self.max_error_bytes)
            .is_none_or(|bytes| bytes > crate::application::MAX_OUTPUT_BYTES)
        {
            return Err(AppError::input(
                "feedback JSON receipts exceed application output admission",
            ));
        }
        Ok(())
    }
}

/// A bounded round's evidence. Success concerns only its original finite
/// entries; a local parametric domain solve is not recursive family closure.
#[derive(Clone, Debug)]
pub struct RoutedFeedbackRoundResult {
    pub completed_finite_trace: bool,
    pub feedback_round_complete: bool,
    pub document: Value,
}

/// Retains actual generated work, shared source definitions and verified maps.
/// Each round starts a fresh graph; it does not reuse a previous seen/memo set.
/// Policy is fixed for the lifetime of the exact installed-ray ledger.
#[derive(Debug)]
pub struct RoutedFeedbackSession<const N: usize> {
    reducer: RoutedCandidateReducer<N>,
    targets: Vec<IntegralKey>,
    workers: usize,
    options: RoutedFeedbackOptions,
    installed: Vec<Ray<N>>,
    rounds: usize,
}

impl<const N: usize> RoutedFeedbackSession<N> {
    /// Prepare owners and routes once using the normal input-driven loader.
    /// N is an API-edge dispatch parameter, checked against the supplied input.
    /// Cancelled preparation returns None, never a usable/complete session.
    pub fn prepare(
        request: RoutedCampaignRequest,
        options: RoutedFeedbackOptions,
        cancellation: &AtomicBool,
        observer: impl Fn(Value),
    ) -> Result<Option<Self>, AppError> {
        options.validate::<N>()?;
        if !(1..=50).contains(&request.workers) {
            return Err(AppError::input("workers must be in 1..=50"));
        }
        rustred::campaign::ParallelExecution::preflight_requested_core_budget(request.workers)
            .map_err(|e| AppError::input(e.to_string()))?;
        let (selection, n, limits) = input::Selection::parse(&request.selection_json)?;
        if N != n {
            return Err(AppError::input("feedback session arity differs from input"));
        }
        let targets = input::targets(
            &request.targets_csv,
            n,
            request.trace_limits.max_input_targets,
        )?;
        observer(
            json!({"event":"feedback_admitted", "arity":N, "targets":targets.len(),
            "workers":request.workers, "source_workers":1, "effective_feedback":format!("{options:?}"),
            "family_closure_claim":false, "durable_overlay_resume":false}),
        );
        let Some(reducer) =
            prepare::prepare::<N>(&request, &selection, limits, cancellation, &observer)?
        else {
            observer(
                json!({"event":"feedback_preparation_cancelled", "completed_finite_trace":false}),
            );
            return Ok(None);
        };
        Ok(Some(Self {
            reducer,
            targets,
            workers: request.workers,
            options,
            installed: Vec::new(),
            rounds: 0,
        }))
    }

    /// Actual immutable programs, including all successful prior publications.
    pub fn programs(&self) -> &Arc<CandidateOwnerPrograms<N>> {
        self.reducer.programs()
    }
    pub fn installed_jobs(&self) -> usize {
        self.installed.len()
    }
    pub fn options(&self) -> RoutedFeedbackOptions {
        self.options
    }

    /// Trace -> bounded real MissingRule ray searches -> append -> retrace.
    /// Errors never nominate source work. Cancellation cannot preempt a native
    /// search; after it returns an uncommitted result is discarded on stop.
    /// Earlier successful publications survive a later failure or cancellation.
    pub fn run_round(
        &mut self,
        cancellation: &AtomicBool,
        observer: impl Fn(Value),
    ) -> RoutedFeedbackRoundResult {
        let started = Instant::now();
        self.rounds = self.rounds.saturating_add(1);
        let round = self.rounds;
        let mut document = json!({"schema":"rustred.owner-feedback-round.json.v1", "event":"feedback_finished",
            "round":round, "family_closure_claim":false, "coefficient_backsubstitution":false,
            "work_checkpoint":false, "graph_memo_reused":false, "shared_rules_sources_routes":true,
            "effective_feedback":format!("{:?}",self.options), "jobs":[], "source_jobs_started":0,
            "local_domains_completed":0, "installed_this_round":0});
        if cancellation.load(Ordering::Relaxed) {
            return self.finish(document, "cancelled", false, false, started, &observer);
        }
        let initial = self.trace("initial_trace", cancellation, &observer);
        document["initial_trace"] = trace_json(&initial, self.options.max_error_bytes);
        let initial = match initial {
            Ok(report) => report,
            Err(_) => {
                return self.finish(
                    document,
                    "initial_trace_incomplete",
                    false,
                    false,
                    started,
                    &observer,
                );
            }
        };
        if cancellation.load(Ordering::Relaxed) {
            return self.finish(document, "cancelled", false, false, started, &observer);
        }
        if initial.trace().frontier().is_empty() {
            return self.finish(
                document,
                "finite_trace_complete",
                true,
                true,
                started,
                &observer,
            );
        }
        observer(json!({"event":"feedback_nomination", "round":round}));
        let nomination = nominate(
            initial.trace(),
            &self.installed,
            self.options.max_jobs_per_round,
            self.options.max_nomination_bytes,
            cancellation,
        );
        document["nomination"] = json!({"jobs":nomination.jobs.len(),
            "duplicate_entries":nomination.duplicate_entries,
            "already_installed_entries":nomination.already_installed_entries,
            "complete":nomination.incomplete.is_none(), "incomplete":nomination.incomplete.as_ref().map(|(reason,target)| json!({"reason":reason,"target":target}))});
        // Release the potentially large completed graph before native search.
        drop(initial);
        let mut receipts = Vec::new();
        if receipts.try_reserve_exact(nomination.jobs.len()).is_err() {
            return self.finish(
                document,
                "receipt_allocation_failed",
                false,
                false,
                started,
                &observer,
            );
        }
        let mut failed = nomination.incomplete.is_some();
        let mut source_jobs = 0usize;
        let mut completed_domains = 0usize;
        let mut installed_this_round = 0usize;
        for (ordinal, ray) in nomination.jobs.iter().enumerate() {
            let mut receipt = json!({"ordinal":ordinal,"owner_mask":mask(&ray.owner),
                "fixed":ray.fixed.as_slice(), "active_coordinates_symbolic":true,
                "actual_numerator_rank":ray.rank,"status":"nominated"});
            if cancellation.load(Ordering::Relaxed) {
                receipt["status"] = json!("cancelled_before_search");
                receipts.push(receipt);
                failed = true;
                break;
            }
            if self.installed.len() >= self.options.max_installed_jobs
                || self.installed.try_reserve_exact(1).is_err()
            {
                receipt["status"] = json!("installed_ledger_limit");
                receipts.push(receipt);
                failed = true;
                break;
            }
            let bound = match self
                .programs()
                .bind_owner_search(ray.owner, self.options.prospective_policy)
            {
                Ok(bound) => bound,
                Err(error) => {
                    receipt["status"] = json!("binding_failed");
                    receipt["error"] = feedback_error(&error, self.options.max_error_bytes);
                    receipts.push(receipt);
                    failed = true;
                    break;
                }
            };
            source_jobs += 1;
            observer(
                json!({"event":"source_search", "round":round,"ordinal":ordinal,"owner_mask":mask(&ray.owner),"actual_numerator_rank":ray.rank}),
            );
            let job_started = Instant::now();
            let mut last_event = Instant::now();
            let result=bound.solve_domains_with_observer(vec![ray.case().into()], OwnerDomainScope {
                max_numerator_rank:Some(ray.rank), finite_case_policy:self.options.finite_case_policy,
            }, self.options.attempt_limits, |event| {
                let (kind, force)=match event {
                    SectorEvent::CaseStarted {..} => ("case_started",true),
                    SectorEvent::PhaseStarted {..} => ("phase_started",true),
                    SectorEvent::RuleFound {..} => ("rule_found",true),
                    SectorEvent::NumericalStarted {..} => ("numerical_started",true),
                    SectorEvent::Search {..} => ("search",false),
                };
                if force || last_event.elapsed() >= Duration::from_millis(250) {
                    observer(json!({"event":"source_progress","round":round,"ordinal":ordinal,"kind":kind,"elapsed_seconds":job_started.elapsed().as_secs_f64()}));
                    last_event=Instant::now();
                }
            });
            receipt["search_seconds"] = json!(job_started.elapsed().as_secs_f64());
            if cancellation.load(Ordering::Relaxed) {
                receipt["status"] = json!("cancelled_after_search_uninstalled");
                receipts.push(receipt);
                failed = true;
                break;
            }
            let result = match result {
                Ok(result) => result,
                Err(error) => {
                    receipt["status"] = json!("search_failed");
                    receipt["error"] = feedback_error(&error, self.options.max_error_bytes);
                    receipts.push(receipt);
                    failed = true;
                    break;
                }
            };
            completed_domains += 1;
            receipt["rules"] = json!(result.rule_count());
            receipt["declared_terminals"] = json!(result.terminal_count());
            receipt["local_domain_complete"] = json!(true);
            observer(
                json!({"event":"local_domain_complete","round":round,"ordinal":ordinal,"rules":result.rule_count(),"declared_terminals":result.terminal_count(),"recursive_coverage_claim":false}),
            );
            let mut raw_limits = self.options.overlay_limits;
            raw_limits.max_native_bytes = raw_limits
                .max_native_bytes
                .min(self.options.max_staged_native_bytes);
            match result.raw_payload_usage(raw_limits) {
                Ok(usage) => receipt["raw_native_bytes"] = json!(usage.native_bytes),
                Err(error) => {
                    receipt["status"] = json!("raw_staging_limit");
                    receipt["error"] = feedback_error(&error, self.options.max_error_bytes);
                    receipts.push(receipt);
                    failed = true;
                    break;
                }
            }
            if cancellation.load(Ordering::Relaxed) {
                receipt["status"] = json!("cancelled_before_append");
                receipts.push(receipt);
                failed = true;
                break;
            }
            observer(json!({"event":"overlay_admission","round":round,"ordinal":ordinal}));
            let next = match self
                .programs()
                .append_domain_overlays(vec![result], self.options.overlay_limits)
            {
                Ok(next) => next,
                Err(error) => {
                    receipt["status"] = json!("append_failed");
                    receipt["error"] = feedback_error(&error, self.options.max_error_bytes);
                    receipts.push(receipt);
                    failed = true;
                    break;
                }
            };
            let next = match self.reducer.with_programs(next) {
                Ok(next) => next,
                Err(error) => {
                    receipt["status"] = json!("rebind_failed");
                    receipt["error"] = bounded_debug(&error, self.options.max_error_bytes);
                    receipts.push(receipt);
                    failed = true;
                    break;
                }
            };
            if cancellation.load(Ordering::Relaxed) {
                receipt["status"] = json!("cancelled_before_publication");
                receipts.push(receipt);
                failed = true;
                break;
            }
            self.reducer = next;
            self.installed.push(*ray);
            installed_this_round += 1;
            receipt["status"] = json!("installed");
            observer(
                json!({"event":"overlay_installed","round":round,"ordinal":ordinal,"installed_jobs":self.installed.len()}),
            );
            receipts.push(receipt);
        }
        let recorded = receipts.len();
        for (ordinal, ray) in nomination.jobs.iter().enumerate().skip(recorded) {
            receipts.push(json!({"ordinal":ordinal,"owner_mask":mask(&ray.owner),
                "fixed":ray.fixed.as_slice(),"actual_numerator_rank":ray.rank,
                "status":if cancellation.load(Ordering::Relaxed) {"cancelled_not_started"} else {"not_started_after_stop"}}));
        }
        document["jobs"] = json!(receipts);
        document["source_jobs_started"] = json!(source_jobs);
        document["local_domains_completed"] = json!(completed_domains);
        document["installed_this_round"] = json!(installed_this_round);
        if cancellation.load(Ordering::Relaxed) {
            return self.finish(document, "cancelled", false, false, started, &observer);
        }
        if installed_this_round == 0 {
            return self.finish(
                document,
                if failed {
                    "feedback_incomplete"
                } else {
                    "frontier_without_new_job"
                },
                false,
                !failed,
                started,
                &observer,
            );
        }
        let final_trace = self.trace("retrace", cancellation, &observer);
        document["final_trace"] = trace_json(&final_trace, self.options.max_error_bytes);
        if cancellation.load(Ordering::Relaxed) {
            return self.finish(document, "cancelled", false, false, started, &observer);
        }
        let complete = final_trace
            .as_ref()
            .is_ok_and(|report| report.trace().frontier().is_empty());
        let round_complete = !failed && final_trace.is_ok();
        self.finish(
            document,
            if !round_complete {
                "feedback_incomplete"
            } else if complete {
                "finite_trace_complete"
            } else {
                "frontier"
            },
            complete,
            round_complete,
            started,
            &observer,
        )
    }

    fn trace(
        &self,
        phase: &str,
        cancellation: &AtomicBool,
        observer: &impl Fn(Value),
    ) -> Result<CandidateRoutedCampaignReport<N>, CandidateRoutedCampaignError<N>> {
        observer(json!({"event":"feedback_phase","phase":phase,"round":self.rounds}));
        self.reducer.trace_targets_parallel_with_observer(
            self.targets.iter().cloned(),
            self.workers,
            cancellation,
            |snapshot| {
                let mut value = feedback_snapshot(snapshot, self.options.max_error_bytes);
                value["feedback_phase"] = json!(phase);
                observer(value);
            },
        )
    }

    fn finish(
        &self,
        mut document: Value,
        status: &str,
        complete: bool,
        round_complete: bool,
        started: Instant,
        observer: &impl Fn(Value),
    ) -> RoutedFeedbackRoundResult {
        document["status"] = json!(status);
        document["completed_finite_trace"] = json!(complete);
        document["feedback_round_complete"] = json!(round_complete);
        document["installed_jobs_total"] = json!(self.installed.len());
        document["prepared_overlay_native_bytes"] =
            json!(self.programs().overlay_usage().native_bytes);
        document["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
        observer(document.clone());
        RoutedFeedbackRoundResult {
            completed_finite_trace: complete,
            feedback_round_complete: round_complete,
            document,
        }
    }
}

fn mask<const N: usize>(mask: &[bool; N]) -> String {
    mask.iter()
        .map(|&active| if active { '1' } else { '0' })
        .collect()
}
fn trace_json<const N: usize>(
    result: &Result<CandidateRoutedCampaignReport<N>, CandidateRoutedCampaignError<N>>,
    cap: usize,
) -> Value {
    let (report, error) = match result {
        Ok(report) => (report, None),
        Err(error) => (
            error.partial_report(),
            Some(bounded_debug(error.reason(), cap)),
        ),
    };
    json!({"traversal_finished":result.is_ok(),"frontier":report.trace().frontier().len(),"snapshot":feedback_snapshot(report.snapshot(),cap),"error":error})
}
fn feedback_snapshot<const N: usize>(
    snapshot: &CandidateRoutedCampaignSnapshot<N>,
    cap: usize,
) -> Value {
    let failure = snapshot.first_failure.as_ref().map(|failure| {
        let mut value = bounded_debug(failure, cap);
        value["kind"] = json!(match failure {
            CandidateRoutedCampaignFailure::Trace(_) => "trace",
            CandidateRoutedCampaignFailure::Cancelled => "cancelled",
            CandidateRoutedCampaignFailure::WorkerPanicked => "worker_panicked",
        });
        value
    });
    snapshot_json_with_failure(snapshot, failure)
}
// Six bytes per input byte covers worst-case JSON escaping. A job's bounded
// scalar/16-axis metadata fits8KiB; two50-worker trace snapshots, policy and
// round metadata fit512KiB. Each trace may repeat an error in its first-failure
// snapshot and error field. This bounds the retained JSON receipt, not observer
// event history, allocator overhead or native working memory.
fn receipt_bound(jobs: usize, error_bytes: usize) -> Option<usize> {
    let escaped = error_bytes.checked_mul(6)?;
    jobs.checked_mul(8192usize.checked_add(escaped)?)?
        .checked_add(escaped.checked_mul(4)?)?
        .checked_add(512 * 1024)
}
fn feedback_error<const N: usize>(error: &OwnerFeedbackError<N>, cap: usize) -> Value {
    let kind = match error {
        OwnerFeedbackError::Candidate(_) => "candidate",
        OwnerFeedbackError::Source(_) => "source",
        OwnerFeedbackError::Search(_) => "search",
        OwnerFeedbackError::InvalidInput(_) => "invalid_input",
        OwnerFeedbackError::ResourceLimit { .. } => "resource_limit",
    };
    let mut value = bounded_debug(error, cap);
    value["kind"] = json!(kind);
    value
}
fn bounded_debug(value: &impl fmt::Debug, cap: usize) -> Value {
    struct Bounded {
        text: String,
        cap: usize,
        truncated: bool,
    }
    impl Write for Bounded {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            let remaining = self.cap.saturating_sub(self.text.len());
            if text.len() <= remaining {
                self.text.push_str(text);
                return Ok(());
            }
            let mut end = remaining;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            self.text.push_str(&text[..end]);
            self.truncated = true;
            Err(fmt::Error)
        }
    }
    let mut out = Bounded {
        text: String::new(),
        cap,
        truncated: false,
    };
    let _ = write!(&mut out, "{value:?}");
    json!({"detail":out.text,"detail_truncated":out.truncated})
}
