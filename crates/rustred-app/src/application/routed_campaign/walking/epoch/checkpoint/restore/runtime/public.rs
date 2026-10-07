//! Checkpoint-enabled public Epoch. Reports deliberately remain checkpoint-only
//! even at drain: no resource-stop path invokes the S2 whole-arena finalizer.
use super::super::super::{
    metadata::{Admission, Identity},
    publication, stop,
};
use super::{Restored, admission, controller, open};
use crate::AppError;
use crate::application::routed_campaign::walking::finite_replay::Account;
use crate::application::routed_campaign::{
    matching::input::Query,
    walking::{
        OwnerDomainWalkEpochInspectorLookup, OwnerDomainWalkG2ResidualAnchors,
        OwnerDomainWalkRecords, OwnerDomainWalkRequest, OwnerDomainWalkResult,
        epoch::{
            self,
            inspector::{Activity, Context, Status},
            ledger6::Tag,
        },
        worker_budget::WorkerBudget,
    },
};
use rustred::solver::RoutedCandidateReducer;
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Request-sized, prepared before the durable state. No arena/root scan in a
/// saved callback, and aliases/source-frontier rows count as query rows.
struct Roles {
    required_prefix: Vec<usize>,
    total: usize,
    original_total: usize,
}
impl Roles {
    fn new(queries: &[Query]) -> io::Result<Self> {
        let mut required_prefix = Vec::new();
        required_prefix
            .try_reserve_exact(
                queries
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("query census range"))?,
            )
            .map_err(|_| io::Error::other("query census allocation"))?;
        required_prefix.push(0);
        for query in queries {
            required_prefix
                .push(required_prefix.last().copied().unwrap() + usize::from(!query.auxiliary));
        }
        let total = required_prefix.len() - 1;
        Ok(Self {
            required_prefix,
            total,
            original_total: total,
        })
    }
    fn append(&mut self, queries: &[Query]) -> io::Result<()> {
        self.required_prefix
            .try_reserve_exact(queries.len())
            .map_err(|_| io::Error::other("extended query census allocation"))?;
        for query in queries {
            self.required_prefix.push(
                self.required_prefix.last().copied().unwrap() + usize::from(!query.auxiliary),
            );
        }
        self.total = self.required_prefix.len() - 1;
        Ok(())
    }
    fn json(&self, admitted: usize) -> Value {
        let admitted = admitted.min(self.total);
        let required = self.required_prefix[self.total];
        let original_required = self.required_prefix[self.original_total];
        let admitted_required = self.required_prefix[admitted];
        json!({"requested":self.total,"admitted":admitted,"unadmitted":self.total-admitted,
            "required":required,"auxiliary":self.total-required,
            "admitted_required":admitted_required,"admitted_auxiliary":admitted-admitted_required,
            "original_requested":self.original_total,"original_required":original_required,
            "appended_requested":self.total-self.original_total,"appended_required":required-original_required,
            "required_closed":null,"required_closed_evaluated":false,
            "scope":"original and append-only query rows, including aliases and source-frontier rows; not the protected initial-domain prefix"})
    }
}

const PHASES: [&str; 10] = [
    "restore",
    "admission",
    "boundary",
    "inspect",
    "p1",
    "p2",
    "p3",
    "checkpoint",
    "drain_wait",
    "joined",
];
struct Telemetry {
    phase: usize,
    since: Instant,
    emitted: Instant,
    wall: [f64; 10],
}
impl Telemetry {
    fn new() -> Self {
        Self {
            phase: 0,
            since: Instant::now(),
            emitted: Instant::now(),
            wall: [0.0; 10],
        }
    }
    fn change(&mut self, phase: &'static str) -> bool {
        self.change_at(phase, Instant::now())
    }
    fn change_at(&mut self, phase: &'static str, now: Instant) -> bool {
        self.wall[self.phase] += now.duration_since(self.since).as_secs_f64();
        self.since = now;
        let next = PHASES
            .iter()
            .position(|name| *name == phase)
            .expect("fixed telemetry phase");
        self.phase = next;
        // Phase accounting is cheap; JSON/observer work is rate limited even
        // when thousands of tiny cuts change phase within one interval.
        if now.duration_since(self.emitted).as_secs_f64() >= 5.0 {
            self.emitted = now;
            true
        } else {
            false
        }
    }
    fn json(&self) -> Value {
        let mut values = serde_json::Map::new();
        for (index, name) in PHASES.iter().enumerate() {
            values.insert((*name).into(), json!(self.wall[index]));
        }
        json!({"phase":PHASES[self.phase],"phase_wall_seconds":values,
            "scope":"current invocation coordinator wall; diagnostics only, not persisted lifetime totals"})
    }
}

/// Observer transport is not mathematical authority. Its panic must not enter
/// the controller's engine-fatal catch and poison an already durable save.
fn emit(observer: &impl Fn(Value), cancel: &AtomicBool, failed: &Cell<bool>, value: Value) {
    if failed.get() {
        return;
    }
    if catch_unwind(AssertUnwindSafe(|| observer(value))).is_err() {
        failed.set(true);
        cancel.store(true, Ordering::Release);
    }
}

fn execution_error<const N: usize>(
    restored: &Restored<N>,
    error: io::Error,
    observer: &impl Fn(Value),
    cancel: &AtomicBool,
    failed: &Cell<bool>,
) -> AppError {
    if restored.state.poisoned {
        emit(
            observer,
            cancel,
            failed,
            json!({"event":"checkpoint_unusable","operation":"owner_domain_walk",
            "checkpoint":{"format":publication::FORMAT,"schema":publication::SCHEMA,
                "directory":restored.publisher.directory(),"generation":restored.publisher.current_generation(),
                "state":"poisoned","resumable":false},"engine_certification_void":true,
            "family_closure_claim":false}),
        );
        AppError::internal_invariant(error.to_string())
    } else {
        AppError::input(error.to_string())
    }
}

fn checkpoint<const N: usize>(restored: &Restored<N>, receipt: &publication::Receipt) -> Value {
    let counts = restored.state.ledger.counts();
    let native =
        counts.get(Tag::Native) + counts.get(Tag::NativeFrontier) + counts.get(Tag::NativeError);
    json!({"format":publication::FORMAT,"schema":publication::SCHEMA,
        "walk_semantics_version":epoch::EPOCH_WALK_SEMANTICS_VERSION,
        "state":"saved","generation":receipt.generation,"manifest":receipt.manifest,
        "manifest_blake3":blake3::Hash::from(receipt.manifest_blake3).to_hex().to_string(),
        "manifest_digest_scope":"canonical manifest object inside authenticated envelope",
        "directory":restored.publisher.directory(),"resumable":true,"saved_this_invocation":true,
        "paused":restored.stop_reason.is_some(),"stop_reason":restored.stop_reason,
        "committed_domains":native+counts.get(Tag::Alias)+counts.get(Tag::Abandoned),"completed_native_inspections":native,
        "abandoned_obligations":counts.get(Tag::Abandoned),
        "committed_events":restored.state.counters.events,"pending_domains":restored.state.pending_or_reserved(),
        "warnings":receipt.warnings})
}

fn scalar_progress<const N: usize>(
    state: &epoch::state::EpochState<N>,
    dispatch: &epoch::dispatch::Dispatch,
) -> Value {
    let counts = state.ledger.counts();
    let native =
        counts.get(Tag::Native) + counts.get(Tag::NativeFrontier) + counts.get(Tag::NativeError);
    json!({"k":state.k,"scheduled_nodes":state.watermark(),"completed_nodes":state.counters.completed,
        "queued_nodes":state.pending_or_reserved(),"events":state.counters.events,
        "committed_events":state.counters.events,"committed_domains":native+counts.get(Tag::Alias)+counts.get(Tag::Abandoned),
        "processed_nodes":native+counts.get(Tag::Alias)+counts.get(Tag::Abandoned),"native_processed_nodes":native,
        "abandoned_obligations":counts.get(Tag::Abandoned),
        "frontiers":state.counters.frontiers,"ledger6":state.ledger.counts().json(),
        "requeue_waiting":dispatch.queued().0,"deferred_waiting":dispatch.queued().1,
        "max_scheduled_finite_rank":state.store.max_finite_rank,
        "unbounded_rank_domains":state.store.unbounded_rank_domains,
        "encountered_numerator_rank":state.store.encountered_rank.json(),
        "descendant_closure":scalar_closure(state)})
}

fn lookup_mode(request: &OwnerDomainWalkRequest) -> controller::LookupMode {
    match request.epoch_inspector_lookup {
        OwnerDomainWalkEpochInspectorLookup::AllMiss => controller::LookupMode::AllMiss,
        OwnerDomainWalkEpochInspectorLookup::Snapshot => controller::LookupMode::Snapshot,
    }
}

/// A fresh bounded pool observation. The outer CLI heartbeat supplies its
/// progress age when repeating this event, so a stalled coordinator does not
/// turn an old callback count into a live utilization claim.
fn activity_json(activity: Option<Activity>, phase: &str) -> Value {
    // Even a just-sampled idle inline boundary may immediately enter a long
    // caller-thread CAS before the next five-second emit gate. Never advertise
    // its latched zero as live computing activity while that call runs.
    let inline_unobservable = activity.is_some_and(|value| value.inline && phase != "joined");
    let known = activity.filter(|_| !inline_unobservable);
    json!({
        "active_workers":known.map(|value| value.computing),
        "computing_workers":known.map(|value| value.computing),
        "queued_inspections":known.map(|value| value.queued),
        "returned_inspections":activity.map(|value| value.returned),
        "finished_uncommitted_domains":activity.map(|value| value.returned + value.escrow_returned),
        "cancelled_queued_inspections":activity.map(|value| value.cancelled_queued),
        "occupied_native_slots":activity.map(|value| value.occupied),
        "workers_joined":phase == "joined",
        "activity_observation":if inline_unobservable {"inline_call_not_pollable"}
            else if activity.is_some() {"coordinator_sample"} else {"unavailable"},
        "activity_observation_age_seconds":activity.map(|_| 0.0),
        "activity_scope":"accepted inspection callbacks, not thread CPU utilization; repeated heartbeat age applies"
    })
}

/// Existing monitor key shape, but reservation fields only. Epoch has no
/// equivalent Ready preparation/duty timings to populate here.
fn reservation_json(budget: WorkerBudget) -> Value {
    json!({"inspection_worker_limit":budget.inspection,
        "lookup_worker_limit":budget.helpers,"coordinator_worker_limit":budget.coordinator,
        "requested_worker_budget":budget.requested,
        "scope":"configured compute reservation, not activity or successfully spawned workers"})
}

/// Scalar subset of Tracker::json, with the same monitor-facing meanings but
/// without its retained-storage census or any closure refresh.
fn scalar_closure<const N: usize>(state: &epoch::state::EpochState<N>) -> Value {
    let closure = state.tracker.counters();
    let total = state.watermark() as usize;
    let available = closure.unavailable.is_none()
        && state.tracker.node_count() == total
        && closure.initial == state.p0 as usize;
    json!({"available":available,"initial_total":state.p0,"total_domains":total,
        "initial_closed":available.then_some(closure.initial_closed),
        "total_closed":available.then_some(closure.total_closed),
        "unresolved_domains":available.then_some(total.saturating_sub(closure.total_closed)),
        "locally_inspected":available.then_some(closure.inspected),
        "dependency_edges":available.then_some(state.tracker.edge_count()),
        "graph_revision":closure.revision,"snapshot_revision":closure.snapshot_revision,
        "snapshot_stale":closure.revision!=closure.snapshot_revision,
        "snapshot_age_seconds":state.tracker.snapshot_age_seconds(),
        "last_refresh_seconds":state.tracker.last_refresh_seconds(),
        "refresh_count":closure.refresh_count,"refresh_seconds":closure.refresh_seconds,
        "scan_history":state.tracker.scan_history_json(),
        "refresh_policy":state.tracker.refresh_policy_json(),
        "reason":if available {None} else {Some(closure.unavailable.as_deref().unwrap_or("dependency domain inventory mismatch"))},
        "closed_counts_are_conservative_lower_bounds":true,"refreshed_for_result":false,
        "retained_storage_estimate_bytes":null,"family_closure_claim":false})
}

/// Keep the ordinary schedule shape unchanged when escrow is disabled. The
/// base window controls publication cuts; extra reservations only allow bounded
/// lookahead, and must not silently enlarge a W=1 publication cut.
fn schedule_json(
    request: &OwnerDomainWalkRequest,
    total_window: usize,
    base_window: usize,
    requested_cut: usize,
) -> Value {
    let cut = if request.epoch_rolling {
        requested_cut.min(base_window)
    } else {
        total_window
    };
    let mut schedule = json!({
        "kind":if request.epoch_rolling {"rolling"} else {"lockstep"},
        "depth":if request.epoch_rolling {total_window.div_ceil(cut)} else {1},
        "b":total_window,"window":base_window,"cut_size":cut,
        "dispatch":request.epoch_dispatch.name(),
        "publication_order":request.epoch_publication_order.report_name(),
    });
    if request.epoch_result_escrow_jobs > 0 {
        schedule["result_escrow_jobs"] = json!(request.epoch_result_escrow_jobs);
        schedule["result_escrow_bytes"] = json!(request.epoch_result_escrow_bytes);
    }
    schedule
}

#[allow(clippy::too_many_arguments)]
fn summary<const N: usize>(
    restored: &Restored<N>,
    request: &OwnerDomainWalkRequest,
    roles: &Roles,
    checkpoint: Value,
    telemetry: &Telemetry,
    drained: bool,
    b: usize,
    base_window: usize,
    started: Instant,
    prepared: f64,
    observer_failed: bool,
) -> Value {
    let state = &restored.state;
    let cut_size = restored.cut_size;
    let counts = state.ledger.counts();
    let native =
        counts.get(Tag::Native) + counts.get(Tag::NativeFrontier) + counts.get(Tag::NativeError);
    let mut doc = json!({"schema":"rustred.owner-domain-walk.json.v6",
        "status":if drained {"incomplete"} else {"stopped"},"stop_reason":restored.stop_reason,
        "all_scheduled_domains_resolved":false,"recursive_worklist_exhausted":drained,
        "family_closure_claim":false,"independent_certification":false,"finalization":"not_evaluated",
        "full_result_in_output_document":false,"full_state_in_checkpoint":true,
        "record_inventory":"authenticated_checkpoint_segments_only","resume_supported":true,
        "checkpoint":checkpoint,"restore_warnings":restored.warnings,
        "publication_policy":"epoch_merge_stream","walk_semantics_version":epoch::EPOCH_WALK_SEMANTICS_VERSION,
        "workers":request.workers,"worker_allocation":crate::application::routed_campaign::walking::worker_budget::WorkerBudget::for_request(request).json(request.inspection_workers)});
    // Keep each macro expansion bounded without changing the summary object.
    let Value::Object(progress) = json!({
        "scheduled_nodes":state.watermark(),"completed_nodes":state.counters.completed,
        "queued_nodes":state.pending_or_reserved(),"processed_nodes":native+counts.get(Tag::Alias)+counts.get(Tag::Abandoned),
        "committed_domains":native+counts.get(Tag::Alias)+counts.get(Tag::Abandoned),"native_processed_nodes":native,
        "abandoned_obligations":counts.get(Tag::Abandoned),
        "failed_nodes":counts.get(Tag::NativeError),"events":state.counters.events,"committed_events":state.counters.events,
        "frontiers":state.counters.frontiers,"input_frontiers_count":restored.roots.frontiers.len(),
        "initial_entry_domains_total":state.p0,"initial_entry_domains_published":null,
        "pending_descendant_domains":null,"contiguous_publication_watermark":null,
        "query_admission":roles.json(restored.roots.rows.len()),"admission_complete":matches!(restored.admission,Admission::Complete),
        "admission_failure":restored.admission_failure,"operational_stop":restored.operational_stop,
        "observer_failed":observer_failed,
        "parallel":{"active_workers":0,"computing_workers":0,"workers_joined":true,
            "queued_inspections":0,"returned_inspections":0,"finished_uncommitted_domains":0,
            "occupied_native_slots":0,"cancelled_queued_inspections":0,
            "activity_observation":"joined","activity_observation_age_seconds":0.0,
            "activity_scope":"accepted inspection callbacks, not thread CPU utilization",
            "inspector_threads":if request.workers==1 {0} else {crate::application::routed_campaign::walking::worker_budget::WorkerBudget::for_request(request).inspection},
            "cancellation":"W1 cooperative caller-thread CAS; W>=2 durable save may precede join"},
        "descendant_closure":scalar_closure(state),
        "epoch":{"stage":"S3_checkpoint_lifecycle","k":state.k,"p0":state.p0,"watermark":state.watermark(),
            "schedule":schedule_json(request,b,base_window,cut_size),"resolution":"canonical_in_merge",
            "records_digest":state.edges.records_digest(),"edge_digest":state.edges.edge_digest(),
            "ledger6":counts.json(),"engine_certification_void":false,"telemetry":telemetry.json()}})
    else {
        unreachable!("summary progress object")
    };
    doc.as_object_mut()
        .expect("summary object")
        .extend(progress);
    doc["max_scheduled_finite_rank"] = json!(state.store.max_finite_rank);
    doc["unbounded_rank_domains"] = json!(state.store.unbounded_rank_domains);
    doc["encountered_numerator_rank"] = state.store.encountered_rank.json();
    doc["parallel"]["admission_preparation"] = reservation_json(WorkerBudget::for_request(request));
    doc["epoch"]["inspector_lookup_mode"] = json!(request.epoch_inspector_lookup.name());
    if request.epoch_rolling {
        doc["epoch"]["rolling_diagnostics"] = json!(restored.rolling_diagnostics);
        doc["epoch"]["rolling_diagnostics_scope"] = json!(
            "current invocation only; selected cuts and polling calls/time, not worker CPU or lifetime totals; prefix waits include waiting for enough ready results; replica publication waits are separate; inline W1 polling executes native work"
        );
    }
    if let Some(rescue) = &state.rescue {
        doc["amendments"] =
            crate::application::routed_campaign::walking::rescue::chain_json(&rescue.amendments);
        doc["rescue"] = json!({"quarantined_domains":epoch::rescue::count(&state.store.quarantine),
            "abandoned_obligations":epoch::rescue::count(&rescue.abandoned),
            "scope":"future lookup exclusion; historical records and dependencies retained", "family_closure_claim":false});
    }
    if let Some(report) = epoch::g2::report(state) {
        doc["g2_residual_anchors"] = report;
    }
    doc["epoch"]["inspector_lookup"] = json!(state.inspector_lookup);
    doc["epoch"]["preparation"] = json!(state.preparation);
    doc["epoch"]["preparation_configuration"] = json!(
        super::super::super::metadata::Preparation::from_request(request)
    );
    doc["epoch"]["preparation_scope"] =
        json!("invocation-local accepted P2 plans; includes later P3 capacity refusal");
    doc["epoch"]["inspector_lookup_scope"] = json!(
        "accepted P2 work in this invocation, including later P3 reservation refusal; excludes rejected or interrupted cuts; not checkpoint lifetime totals"
    );
    doc["epoch"]["coordinator_miss_rechecks_skipped_scope"] = json!(
        "full Store::lookup calls avoided; not candidate comparisons, nonempty index probes or measured time"
    );
    crate::application::routed_campaign::walking::finish_timing(&mut doc, started, prepared);
    doc
}

#[allow(clippy::too_many_arguments)]
pub(in crate::application::routed_campaign::walking::epoch) fn run<const N: usize>(
    request: &OwnerDomainWalkRequest,
    reducer: &RoutedCandidateReducer<N>,
    owners: &[String],
    queries: &[Query],
    b: usize,
    started: Instant,
    prepared: f64,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<OwnerDomainWalkResult, AppError> {
    let options = request
        .checkpoint
        .as_ref()
        .ok_or_else(|| AppError::input("CP6 requires explicit checkpoint directory"))?;
    let b = request
        .resolved_epoch_total_window(b)
        .map_err(AppError::input)?;
    let identity = Identity::new(request, owners, queries)
        .map_err(|error| AppError::input(error.to_string()))?;
    let mut roles = Roles::new(queries).map_err(|error| AppError::input(error.to_string()))?;
    for amendment in identity.amendments() {
        roles
            .append(&amendment.queries)
            .map_err(|error| AppError::input(error.to_string()))?;
    }
    let failed = Cell::new(false);
    let telemetry = RefCell::new(Telemetry::new());
    let reservations = reservation_json(WorkerBudget::for_request(request));
    if !options.resume {
        telemetry.borrow_mut().change("admission");
    }
    emit(
        observer,
        cancellation,
        &failed,
        json!({"event":"epoch_lifecycle","operation":"owner_domain_walk",
        "phase":if options.resume {"restore"} else {"fresh"},
        "epoch_inspector_lookup":request.epoch_inspector_lookup.name(),"family_closure_claim":false}),
    );
    let mut restored = if options.resume {
        open(options.directory.clone(), &identity, reducer, b)
    } else {
        admission::fresh(options.directory.clone(), &identity)
    }
    .map_err(|error| AppError::input(error.to_string()))?;
    let b = if options.resume && request.epoch_rolling {
        restored.window
    } else {
        b
    };
    identity
        .epoch_base_window(b)
        .map_err(|error| AppError::input(error.to_string()))?;
    restored.window = b;
    let last = RefCell::new(None::<Value>);
    let progress = |state: &epoch::state::EpochState<N>,
                    dispatch: &epoch::dispatch::Dispatch,
                    phase,
                    activity: &dyn Fn() -> Option<Activity>| {
        let mut time = telemetry.borrow_mut();
        if time.change(phase) {
            let mut event = scalar_progress(state, dispatch);
            event["event"] = json!("epoch_heartbeat");
            event["operation"] = json!("owner_domain_walk");
            event["phase"] = json!(phase);
            event["telemetry"] = time.json();
            event["parallel"] = activity_json(activity(), phase);
            event["parallel"]["admission_preparation"] = reservations.clone();
            event["family_closure_claim"] = json!(false);
            emit(observer, cancellation, &failed, event);
        }
    };
    let saved = |restored: &Restored<N>, receipt: &publication::Receipt, status: &[Status]| {
        let metadata = checkpoint(restored, receipt);
        *last.borrow_mut() = Some(metadata.clone());
        let returned = status.iter().filter(|entry| entry.returned).count();
        let started_count = status.iter().filter(|entry| entry.started).count();
        let mut event = scalar_progress(&restored.state, &restored.dispatch);
        event["event"] = json!("checkpoint_saved");
        event["operation"] = json!("owner_domain_walk");
        event["checkpoint"] = metadata;
        event["full_result_in_output_document"] = json!(false);
        event["full_state_in_checkpoint"] = json!(true);
        event["query_admission"] = roles.json(restored.roots.rows.len());
        event["parallel"] = json!({"active_workers":null,"workers_joined":false,"observed_cut_started":started_count,
            "observed_cut_returned":returned,"observed_cut_descriptors":status.len(),
            "activity_observation":"saved_cut_observation_not_live","activity_observation_age_seconds":null});
        event["parallel"]["admission_preparation"] = reservations.clone();
        event["family_closure_claim"] = json!(false);
        emit(observer, cancellation, &failed, event);
    };
    let admission_stopped = if matches!(restored.admission, Admission::InProgress) {
        matches!(
            admission::continue_observed(
                &mut restored,
                &identity,
                reducer,
                b,
                || stop::requested(cancellation, request.epoch_stop_file.as_deref()),
                |restored, receipt| saved(restored, receipt, &[]),
                epoch::admission::one,
                |state, dispatch| progress(state, dispatch, "admission", &|| None)
            )
            .map_err(|error| execution_error(
                &restored,
                error,
                observer,
                cancellation,
                &failed
            ))?,
            admission::Outcome::Stopped(_)
        )
    } else {
        false
    };
    let finite_account = Account::default();
    let drained = if admission_stopped {
        false
    } else {
        super::rescue::apply(
            &mut restored,
            &identity,
            reducer,
            b,
            || stop::requested(cancellation, request.epoch_stop_file.as_deref()).is_some(),
            &mut |restored, receipt| saved(restored, receipt, &[]),
        )
        .map_err(|error| AppError::input(error.to_string()))?;
        // Reuse is computed only over validated protected p0, including on a
        // resumed nonzero merge. Do not admit a worker merely because it built.
        let overlap = match admission::resumed_overlap(&restored, request, cancellation, b) {
            Ok(overlap) => overlap,
            Err(error)
                if error.kind() == io::ErrorKind::Interrupted
                    || error.to_string().contains("allocation") =>
            {
                let context = stop::requested(cancellation, request.epoch_stop_file.as_deref());
                let reason = context.as_ref().map_or(
                    if error.kind() == io::ErrorKind::Interrupted {
                        epoch::merge::StopReason::Paused
                    } else {
                        epoch::merge::StopReason::RamGuard
                    },
                    stop::Stop::kind,
                );
                let receipt = controller::save_observed(
                    &mut restored,
                    &identity,
                    b,
                    Some(reason),
                    context,
                    &mut |state, dispatch, phase| progress(state, dispatch, phase, &|| None),
                )
                .map_err(|failure| {
                    let error = match failure {
                        controller::Failure::Save(error) => error,
                        controller::Failure::Engine(reason) => {
                            restored.state.poisoned = true;
                            let error = match restored.publisher.poison(restored.state.k, &reason) {
                                Ok(()) => reason,
                                Err(error) => format!("{reason}; durable poison failed: {error}"),
                            };
                            io::Error::other(error)
                        }
                    };
                    execution_error(&restored, error, observer, cancellation, &failed)
                })?;
                saved(&restored, &receipt, &[]);
                return finish(
                    &mut restored,
                    request,
                    &identity,
                    &roles,
                    last.into_inner(),
                    &telemetry.into_inner(),
                    false,
                    b,
                    started,
                    prepared,
                    failed.get(),
                    &finite_account,
                    observer,
                    cancellation,
                );
            }
            Err(error) => return Err(AppError::input(error.to_string())),
        };
        if request.g2_residual_anchors == OwnerDomainWalkG2ResidualAnchors::Union
            && restored.state.g2_store.is_none()
        {
            epoch::g2::enable(&mut restored.state);
        }
        let g2_store = restored.state.g2_store.clone();
        let context = Context {
            reducer,
            request,
            overlap: &overlap,
            cancellation,
            g2: g2_store.as_deref(),
            finite_account: request.finite_replay.map(|_| &finite_account),
        };
        let outcome = controller::run_native_observed(
            &mut restored,
            &identity,
            b,
            &context,
            lookup_mode(request),
            saved,
            progress,
        )
        .map_err(|error| execution_error(&restored, error, observer, cancellation, &failed))?;
        matches!(outcome, controller::Outcome::Drained)
    };
    finish(
        &mut restored,
        request,
        &identity,
        &roles,
        last.into_inner(),
        &telemetry.into_inner(),
        drained,
        b,
        started,
        prepared,
        failed.get(),
        &finite_account,
        observer,
        cancellation,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish<const N: usize>(
    restored: &mut Restored<N>,
    request: &OwnerDomainWalkRequest,
    identity: &Identity<'_>,
    roles: &Roles,
    receipt: Option<Value>,
    telemetry: &Telemetry,
    drained: bool,
    b: usize,
    started: Instant,
    prepared: f64,
    observer_failed: bool,
    finite_account: &Account,
    observer: &impl Fn(Value),
    cancellation: &AtomicBool,
) -> Result<OwnerDomainWalkResult, AppError> {
    let receipt = receipt
        .ok_or_else(|| AppError::internal_invariant("CP6 outcome without a durable receipt"))?;
    let evaluate_required = drained && restored.state.rescue.is_some();
    if evaluate_required {
        restored.state.tracker.refresh(cancellation, true);
    }
    let mut document = summary(
        restored,
        request,
        roles,
        receipt,
        telemetry,
        drained,
        b,
        identity
            .epoch_base_window(b)
            .map_err(|error| AppError::internal_invariant(error.to_string()))?,
        started,
        prepared,
        observer_failed,
    );
    if request.finite_replay.is_some() {
        document["finite_replay"] = finite_account.report();
        document["resume_supported"] = false.into();
    }
    if evaluate_required {
        let rows = identity.query_rows(
            restored
                .state
                .rescue
                .as_ref()
                .map_or(0, |r| r.amendments.len()),
        );
        let ids = rows
            .iter()
            .map(|(q, _)| (q.id.as_str(), q.auxiliary))
            .collect::<Vec<_>>();
        let domains = rows
            .iter()
            .map(
                |(q, _)| crate::application::routed_campaign::walking::queue::Domain {
                    phase: crate::application::routed_campaign::walking::queue::Phase::Apply,
                    owner: rustred::arity::storage_array(&q.owner, false)
                        .expect("validated query arity"),
                    lower: rustred::arity::storage_array::<_, N>(&q.lower, 0)
                        .expect("validated query arity")
                        .to_vec(),
                    upper: rustred::arity::storage_array::<_, N>(&q.upper, Some(0))
                        .expect("validated query arity")
                        .to_vec(),
                    rank: q.rank,
                    powers: q.powers,
                },
            )
            .collect::<Vec<_>>();
        let coverage = crate::application::routed_campaign::walking::rescue::query_certification(
            &ids,
            &domains,
            &restored.roots.rows,
            |id| {
                restored
                    .state
                    .store
                    .domains
                    .get(id)
                    .map(|image| image.expand())
            },
            |id| restored.state.tracker.closed(id),
        );
        document["query_admission"]["required_closed"] =
            coverage["required_queries_certified"].clone();
        document["query_admission"]["required_closed_evaluated"] = true.into();
        document["required_queries_resolved"] =
            (coverage["required_queries_total"] == coverage["required_queries_certified"]).into();
        document["query_certification"] = coverage;
        document["finalization"] = "required_query_coverage_evaluated_not_independent".into();
        document["descendant_closure"]["refreshed_for_result"] = true.into();
    }
    let failed = Cell::new(observer_failed);
    emit(
        observer,
        cancellation,
        &failed,
        OwnerDomainWalkResult::completion_progress(&document),
    );
    document["observer_failed"] = json!(failed.get());
    Ok(OwnerDomainWalkResult {
        all_scheduled_domains_resolved: false,
        document,
        records: OwnerDomainWalkRecords::default(),
    })
}

#[cfg(test)]
mod tests;
