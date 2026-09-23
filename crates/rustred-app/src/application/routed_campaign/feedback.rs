//! Opt-in, retained source-feedback sessions. No automatic trace-only mutation.
mod nomination;
mod witness;
pub use witness::{
    RoutedEntryWitnessLimits, RoutedEntryWitnessProposal, RoutedEntryWitnessRoundResult,
    RoutedEntryWitnessStats,
};
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

use super::{RoutedCampaignRequest, entry, input, prepare, snapshot_json_with_failure};
use crate::AppError;
use nomination::{SourceCase, batches, nominate};

/// Scope of a real missing-rule search. Neither mode invents missing targets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RoutedFeedbackNomination {
    /// Preserve the historical positive-parametric ray behavior.
    #[default]
    PositiveRays,
    /// Fix every index to its observed integer value; batch by owner.
    FixedTargets,
}
impl RoutedFeedbackNomination {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PositiveRays => "positive-rays",
            Self::FixedTargets => "fixed-targets",
        }
    }
}

/// Explicit handling of residuals from a successfully finished fixed search.
/// This never turns failed, cancelled or unfinished search work into terminals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RoutedFeedbackFixedResidualPolicy {
    /// Do not publish an overlay containing unsolved fixed targets.
    #[default]
    KeepUnresolved,
    /// Admit searched residuals as a nonminimal finite output convention.
    /// Requires SearchFinite and a positive numerical seed-depth allowance;
    /// no independence, numerical value or universal coverage is established.
    DeclareSearchedFiniteTerminals,
}
impl RoutedFeedbackFixedResidualPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KeepUnresolved => "keep-unresolved",
            Self::DeclareSearchedFiniteTerminals => "declare-searched-finite-terminals",
        }
    }
}

/// Explicit policy for NEW source work. It never authenticates historical
/// artifact settings. One native source job runs at a time, between trace pools.
#[derive(Clone, Copy, Debug)]
pub struct RoutedFeedbackOptions {
    pub prospective_policy: OwnerFeedbackPolicy,
    pub finite_case_policy: FiniteCasePolicy,
    pub nomination: RoutedFeedbackNomination,
    /// Applies only to FixedTargets. PositiveRays keeps its existing policy.
    pub fixed_residual_policy: RoutedFeedbackFixedResidualPolicy,
    pub attempt_limits: OwnerDomainAttemptLimits,
    /// Cumulative installed-overlay limits, independent of raw staging.
    pub overlay_limits: OwnerOverlayLimits,
    /// Maximum nominated cases; fixed points sharing an owner are batched.
    pub max_jobs_per_round: usize,
    /// Logical compact nomination storage; excludes allocator overhead.
    pub max_nomination_bytes: usize,
    /// Cumulative installed-case ledger across every round of this session.
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
            nomination: Default::default(),
            fixed_residual_policy: Default::default(),
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
        if self.nomination == RoutedFeedbackNomination::FixedTargets {
            if self.finite_case_policy != FiniteCasePolicy::SearchFinite {
                return Err(AppError::input(
                    "fixed-target feedback requires SearchFinite",
                ));
            }
            if self.fixed_residual_policy
                == RoutedFeedbackFixedResidualPolicy::DeclareSearchedFiniteTerminals
                && self.prospective_policy.numerical_depth == 0
            {
                return Err(AppError::input(
                    "declaring searched fixed terminals requires positive numerical seed depth",
                ));
            }
        }
        if self.max_jobs_per_round == 0
            || self.max_installed_jobs == 0
            || self.max_staged_native_bytes == 0
            || self.max_error_bytes == 0
            || self.max_nomination_bytes < std::mem::size_of::<SourceCase<N>>()
            || self.attempt_limits.max_requested_cases == 0
        {
            return Err(AppError::input(
                "feedback resource limits must admit at least one job",
            ));
        }
        self.max_installed_jobs
            .checked_mul(std::mem::size_of::<SourceCase<N>>())
            .ok_or_else(|| AppError::input("feedback ledger byte count overflow"))?;
        self.validate_receipt(0)
    }

    fn validate_receipt(&self, entry_description_bytes: usize) -> Result<(), AppError> {
        if receipt_bound(self.max_jobs_per_round, self.max_error_bytes)
            .and_then(|bytes| bytes.checked_add(entry_description_bytes))
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
/// A complete trace is relative to the explicitly selected terminal convention,
/// not proof of terminal independence or numerical evaluability.
#[derive(Clone, Debug)]
pub struct RoutedFeedbackRoundResult {
    pub completed_finite_trace: bool,
    pub feedback_round_complete: bool,
    pub document: Value,
}

/// Retains actual generated work, shared source definitions and verified maps.
/// Each round starts a fresh graph; it does not reuse a previous seen/memo set.
/// Policy is fixed for the lifetime of the exact installed-case ledger.
#[derive(Debug)]
pub struct RoutedFeedbackSession<const N: usize> {
    reducer: RoutedCandidateReducer<N>,
    targets: Vec<IntegralKey>,
    entry_domain: Option<entry::RequestedEntryDomain<N>>,
    workers: usize,
    options: RoutedFeedbackOptions,
    installed: Vec<SourceCase<N>>,
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
        let entry_domain = request
            .entry_domains_json
            .as_deref()
            .map(entry::RequestedEntryDomain::<N>::parse)
            .transpose()?;
        options.validate_receipt(
            entry_domain
                .as_ref()
                .map_or(0, |domain| domain.description_json_bytes()),
        )?;
        if let Some(domain) = &entry_domain {
            for target in &targets {
                domain.validate(target)?;
            }
        }
        observer(
            json!({"event":"feedback_admitted", "arity":N, "targets":targets.len(),
            "workers":request.workers, "source_workers":1, "effective_feedback":format!("{options:?}"),
            "entry_admission":entry::description(entry_domain.as_ref()),
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
            entry_domain,
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

    /// Atomically admit the next finite input batch without reloading programs
    /// or discarding overlays, the installed-case ledger, policy or round count.
    /// Every input (including duplicates) consumes the original trace input cap.
    /// Keys are already exact integers; this checks nonempty batch, arity and
    /// the explicit finite policy when supplied. Source conditions, default
    /// saved-rank admission and rule applicability remain native trace checks.
    /// Any admission/allocation error leaves the previous batch untouched.
    pub fn replace_targets(
        &mut self,
        targets: impl IntoIterator<Item = IntegralKey>,
    ) -> Result<(), AppError> {
        self.options.validate::<N>()?;
        let limit = self.reducer.limits().max_input_targets;
        let mut admitted = Vec::new();
        for target in targets {
            if target.powers().len() != N {
                return Err(AppError::input(
                    "feedback target arity differs from session",
                ));
            }
            if let Some(domain) = &self.entry_domain {
                domain.validate(&target)?;
            }
            let next = admitted
                .len()
                .checked_add(1)
                .ok_or_else(|| AppError::limit("feedback input target count overflow"))?;
            if next > limit {
                return Err(AppError::limit("feedback input targets exceed trace limit"));
            }
            admitted
                .try_reserve_exact(1)
                .map_err(|_| AppError::limit("feedback target batch allocation failed"))?;
            admitted.push(target);
        }
        if admitted.is_empty() {
            return Err(AppError::input("feedback target batch is empty"));
        }
        self.targets = admitted;
        Ok(())
    }

    /// Trace -> bounded real MissingRule searches -> append -> retrace.
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
            "round":round, "input_targets":self.targets.len(),
            "entry_admission":entry::description(self.entry_domain.as_ref()),
            "saved_generation_max_numerator_rank":self.programs().context().scope().max_numerator_rank,
            "family_closure_claim":false, "coefficient_backsubstitution":false,
            "work_checkpoint":false, "graph_memo_reused":false, "shared_rules_sources_routes":true,
            "effective_feedback":format!("{:?}",self.options),
            "nomination_policy":self.options.nomination.as_str(),
            "fixed_residual_policy":self.options.fixed_residual_policy.as_str(),
            "terminal_independence_claim":false,"numerical_terminal_values_claim":false,
            "jobs":[], "source_jobs_started":0,
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
            self.options.nomination,
            self.options.max_jobs_per_round,
            self.options.max_nomination_bytes,
            cancellation,
        );
        document["nomination"] = json!({"jobs":nomination.jobs.len(), "cases":nomination.jobs.len(),
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
        for (ordinal, batch) in batches(
            &nomination.jobs,
            self.options.nomination,
            self.options.attempt_limits.max_requested_cases,
        )
        .enumerate()
        {
            let owner = batch[0].owner;
            let rank = batch
                .iter()
                .map(|case| case.rank)
                .max()
                .expect("nonempty batch");
            let mut receipt = job_receipt(ordinal, batch, self.options.nomination);
            if cancellation.load(Ordering::Relaxed) {
                receipt["status"] = json!("cancelled_before_search");
                receipts.push(receipt);
                failed = true;
                break;
            }
            if self
                .installed
                .len()
                .checked_add(batch.len())
                .is_none_or(|n| n > self.options.max_installed_jobs)
                || self.installed.try_reserve_exact(batch.len()).is_err()
            {
                receipt["status"] = json!("installed_ledger_limit");
                receipts.push(receipt);
                failed = true;
                break;
            }
            let bound = match self
                .programs()
                .bind_owner_search(owner, self.options.prospective_policy)
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
            let mut cases = Vec::new();
            if cases.try_reserve_exact(batch.len()).is_err() {
                receipt["status"] = json!("case_allocation_failed");
                receipts.push(receipt);
                failed = true;
                break;
            }
            cases.extend(batch.iter().map(|case| case.case().into()));
            source_jobs += 1;
            observer(
                json!({"event":"source_search", "round":round,"ordinal":ordinal,"owner_mask":mask(&owner),"actual_numerator_rank":rank,"cases":batch.len()}),
            );
            let job_started = Instant::now();
            let mut last_event = Instant::now();
            let result=bound.solve_domains_with_observer(cases, OwnerDomainScope {
                max_numerator_rank:Some(rank), finite_case_policy:self.options.finite_case_policy,
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
            completed_domains += batch.len();
            receipt["rules"] = json!(result.rule_count());
            receipt["search_residuals"] = json!(result.terminal_count());
            receipt["numerical_cases_searched"] = json!(result.stats().numerical_cases);
            receipt["symbolic_cases_searched"] = json!(result.stats().symbolic_cases);
            receipt["productive_rule_repair"] = json!(result.rule_count() != 0);
            receipt["declared_terminals"] = json!(0);
            receipt["local_domain_complete"] = json!(true);
            if self.options.nomination == RoutedFeedbackNomination::FixedTargets {
                // A successful fixed search leaves only nominated numerical
                // points as residuals. Check that boundary before formatting
                // or accepting a finite terminal convention.
                let residuals = &result.partial_solution().finite_residuals;
                if residuals.len() > batch.len()
                    || residuals.iter().any(|residual| {
                        residual.powers().iter().any(|power| power.is_symbolic())
                            || !batch.iter().any(|case| case.case().integral() == *residual)
                    })
                {
                    receipt["status"] = json!("unexpected_fixed_search_residual");
                    receipts.push(receipt);
                    failed = true;
                    break;
                }
                receipt["searched_residual_keys"] = json!(
                    residuals
                        .iter()
                        .map(|residual| residual
                            .powers()
                            .iter()
                            .map(|power| power.value())
                            .collect::<Vec<_>>())
                        .collect::<Vec<_>>()
                );
                receipt["residual_search_depth"] =
                    json!(self.options.prospective_policy.numerical_depth);
                receipt["residual_policy"] = json!(self.options.fixed_residual_policy.as_str());
                receipt["residual_origin"] = json!("completed-bounded-fixed-source-search");
                if !residuals.is_empty()
                    && self.options.fixed_residual_policy
                        == RoutedFeedbackFixedResidualPolicy::KeepUnresolved
                {
                    // The native append service declares every retained
                    // residual. Keep this whole overlay uninstalled instead
                    // of silently changing that service or claiming closure.
                    receipt["status"] = json!("fixed_search_residuals_uninstalled");
                    receipts.push(receipt);
                    failed = true;
                    break;
                }
            }
            observer(
                json!({"event":"local_domain_complete","round":round,"ordinal":ordinal,"rules":result.rule_count(),"search_residuals":result.terminal_count(),"recursive_coverage_claim":false}),
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
            let declared_terminals = result.terminal_count();
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
            self.installed.extend_from_slice(batch);
            installed_this_round += batch.len();
            receipt["declared_terminals"] = json!(declared_terminals);
            receipt["status"] = json!("installed");
            observer(
                json!({"event":"overlay_installed","round":round,"ordinal":ordinal,"installed_jobs":self.installed.len()}),
            );
            receipts.push(receipt);
        }
        let recorded = receipts.len();
        for (ordinal, batch) in batches(
            &nomination.jobs,
            self.options.nomination,
            self.options.attempt_limits.max_requested_cases,
        )
        .enumerate()
        .skip(recorded)
        {
            let mut receipt = job_receipt(ordinal, batch, self.options.nomination);
            receipt["status"] = json!(if cancellation.load(Ordering::Relaxed) {
                "cancelled_not_started"
            } else {
                "not_started_after_stop"
            });
            receipts.push(receipt);
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
        self.reducer
            .trace_targets_parallel_with_entry_admission_and_observer(
                self.targets.iter().cloned(),
                entry::admission(self.entry_domain.as_ref()),
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

fn job_receipt<const N: usize>(
    ordinal: usize,
    batch: &[SourceCase<N>],
    policy: RoutedFeedbackNomination,
) -> Value {
    let first = &batch[0];
    let mut value = json!({"ordinal":ordinal,"owner_mask":mask(&first.owner),
        "active_coordinates_symbolic":policy == RoutedFeedbackNomination::PositiveRays,
        "actual_numerator_rank":batch.iter().map(|case| case.rank).max().unwrap(),
        "cases":batch.len(),"status":"nominated"});
    if policy == RoutedFeedbackNomination::PositiveRays {
        value["fixed"] = json!(first.fixed.as_slice());
    } else {
        value["fixed_targets"] = json!(
            batch
                .iter()
                .map(|case| case.fixed.as_slice())
                .collect::<Vec<_>>()
        );
        value["target_numerator_ranks"] =
            json!(batch.iter().map(|case| case.rank).collect::<Vec<_>>());
    }
    value
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
// scalar/16-axis metadata fits8KiB, including its fixed input/residual key.
// Fixed batches share error and owner fields: count input cases, not batches.
// Two50-worker trace snapshots, policy and
// round metadata fit512KiB. The explicit entry-domain description is charged
// separately once at immutable-session admission. Each trace may repeat an error in its first-failure
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
