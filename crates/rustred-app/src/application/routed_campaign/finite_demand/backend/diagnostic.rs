//! Test-only durable driver. No public frontend or publication authority.
use super::super::lifecycle::policy::{ExportLimits, RawLimits, StoreLimits};
use super::super::trace::{CensusLimits, SeedLimits, WorkLimits};
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{BufWriter, Read, Write},
    rc::Rc,
    sync::Arc,
    time::Instant,
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mode {
    Create,
    Resume,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Steering {
    mode: Mode,
    checkpoint: PathBuf,
    /// None runs to full scope; Some(0) persists/opens pending state only.
    /// A grouped step may cross this boundary: committed groups stay atomic.
    stop_after_committed_windows: Option<u64>,
    workers: usize,
    #[serde(default = "super::super::lifecycle::policy::one_window")]
    max_windows_per_batch: usize,
    #[serde(default = "super::super::lifecycle::policy::persistent_workers")]
    reuse_worker_pool: bool,
    #[serde(default)]
    parallel_window_preparation: bool,
    windows: SeedLimits,
    per_batch_work: WorkLimits,
    prepared_reduction_aggregates: Option<ReductionAggregateOverrides>,
    census: CensusLimits,
    export: ExportLimits,
    storage: StoreLimits,
    #[serde(default)]
    max_resident_work_bytes: usize,
    max_total_rows: usize,
    max_source_bytes: usize,
    /// All census JSONL and result.json bytes; receipt has a separate 1 MiB cap.
    max_output_bytes: u64,
}
impl Steering {
    fn policy(&self) -> Policy {
        Policy {
            workers: self.workers,
            max_windows_per_batch: self.max_windows_per_batch,
            reuse_worker_pool: self.reuse_worker_pool,
            parallel_window_preparation: self.parallel_window_preparation,
            windows: self.windows.into(),
            per_batch_work: self.per_batch_work.into(),
            prepared_reduction_aggregates: self.prepared_reduction_aggregates,
            census: self.census.into(),
            export: self.export.into(),
            storage: self.storage.into(),
            max_resident_work_bytes: self.max_resident_work_bytes,
            max_total_rows: self.max_total_rows,
            max_source_bytes: self.max_source_bytes,
        }
    }
}

pub(super) fn reader_statistics(reader: &storage::WorkReader) -> Value {
    match reader.statistics() {
        Ok(s) => json!({"page_reads":s.page_reads,"page_bytes_read":s.page_bytes_read,
            "page_cache_hits":s.page_cache_hits,"evictions":s.evictions,
            "uncached_large_pages":s.uncached_large_pages,"retained_pages":s.retained_pages,
            "retained_page_payload_bytes":s.retained_page_payload_bytes,
            "retained_cache_metadata_bytes":s.retained_cache_metadata_bytes,
            "convenience_lookup_calls":s.lookups,
            "native_directory_lookup_count":null}),
        Err(e) => json!({"statistics_error":e.to_string()}),
    }
}

fn hex(digest: [u8; 32]) -> String {
    blake3::Hash::from(digest).to_hex().to_string()
}
fn execution_error(error: impl ToString) -> AppError {
    AppError::execution(error.to_string())
}
fn stream_error(error: impl ToString, resource: bool) -> storage::StorageError {
    use rustred::persistence::finite_work::{GeneratedWorkError, GeneratedWorkErrorKind};
    GeneratedWorkError::new(
        if resource {
            GeneratedWorkErrorKind::Resource
        } else {
            GeneratedWorkErrorKind::Io
        },
        error,
    )
    .into()
}

/// One cumulative byte counter shared by the three independently buffered files.
pub(crate) struct OutputBudget {
    used: Rc<Cell<u64>>,
    limit: u64,
}
impl OutputBudget {
    pub(crate) fn new(limit: u64) -> Self {
        Self {
            used: Rc::new(Cell::new(0)),
            limit,
        }
    }
    fn file(&self, path: &Path) -> Result<EvidenceFile, AppError> {
        Ok(EvidenceFile {
            output: BufWriter::new(
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(execution_error)?,
            ),
            used: Rc::clone(&self.used),
            limit: self.limit,
            digest: blake3::Hasher::new(),
            bytes: 0,
            records: 0,
        })
    }
    pub(crate) fn document(&self, path: &Path, value: &Value) -> Result<(), AppError> {
        let mut file = self.file(path)?;
        file.record(value).map_err(storage_error)?;
        file.finish()?;
        Ok(())
    }
}
struct EvidenceFile {
    output: BufWriter<File>,
    used: Rc<Cell<u64>>,
    limit: u64,
    digest: blake3::Hasher,
    bytes: u64,
    records: u64,
}
impl EvidenceFile {
    fn record(&mut self, value: &(impl Serialize + ?Sized)) -> Result<(), storage::StorageError> {
        // One key/identity (or bounded metadata document), never the full zero set.
        let mut bytes = serde_json::to_vec(value).map_err(|e| stream_error(e, false))?;
        bytes.push(b'\n');
        let len = u64::try_from(bytes.len()).map_err(|e| stream_error(e, true))?;
        let total = self
            .used
            .get()
            .checked_add(len)
            .ok_or_else(|| stream_error("diagnostic output byte overflow", true))?;
        if total > self.limit {
            return Err(stream_error("diagnostic output byte grant", true));
        }
        self.output
            .write_all(&bytes)
            .map_err(|e| stream_error(e, false))?;
        self.used.set(total);
        self.bytes += len;
        self.records += 1;
        self.digest.update(&bytes);
        Ok(())
    }
    fn finish(mut self) -> Result<Value, AppError> {
        self.output.flush().map_err(execution_error)?;
        Ok(json!({"records":self.records,"bytes":self.bytes,
            "blake3":self.digest.finalize().to_hex().to_string()}))
    }
}

fn progress_json(p: &Progress) -> Value {
    let b = p.policy.per_batch_work;
    let r = p.effective_reduction;
    json!({"required_rows":p.required_rows,"exhausted_required_rows":p.exhausted_required_rows,
        "auxiliary_rows":p.auxiliary_rows,"exhausted_auxiliary_rows":p.exhausted_auxiliary_rows,
        "required_complete":p.required_complete,"all_rows_exhausted":p.all_rows_exhausted,
        "no_active_native_work":p.no_active_native_work,"stopped":p.stopped,
        "input_occurrences":p.input_occurrences,"completed_work":p.completed_work,
        "current_row":p.current_row,"current_window":p.current_window,
        "max_windows_per_batch":p.policy.max_windows_per_batch,
        "generation":hex(p.generation),"generation_number":p.generation_number,
        "checkpoint":p.checkpoint,
        "source_reduction_limits":format!("{:?}",p.source_reduction),
        "effective_reduction_limits":format!("{:?}",p.effective_reduction),
        "effective_batch_aggregate_limits":{
            "max_input_targets":p.policy.windows.max_seed_points.saturating_mul(p.policy.max_windows_per_batch).min(b.max_nodes),
            "max_unique_nodes":b.max_nodes,
            "max_rule_applications":r.max_rule_applications.min(b.max_rule_applications),
            "max_pending_frames":r.max_pending_frames.min(b.max_nodes),
            "max_coalescing_additions":r.max_coalescing_additions.min(b.max_coalescing_additions),
            "max_transport_calls":b.max_transport_calls,
            "max_transport_operations":b.max_transport_operations,
            "max_transport_endpoints":b.max_transport_endpoints}})
}

fn stream_census<const N: usize>(
    session: &Session<N>,
    output: &Path,
    budget: &OutputBudget,
    cancel: &AtomicBool,
) -> Result<Value, AppError> {
    let mut terminals = budget.file(&output.join("terminals.jsonl"))?;
    let mut zeros = budget.file(&output.join("zeros.jsonl"))?;
    let mut selections = budget.file(&output.join("selections.jsonl"))?;
    let summary = session.authenticated.visit_raw_census(
        session.policy.storage.raw,
        cancel,
        |kind, key| match kind {
            storage::RawKeyKind::Terminal => terminals.record(key),
            storage::RawKeyKind::Zero => zeros.record(key),
        },
    )?;
    let mut multiplicity = 0u64;
    for row in &session.authenticated.generation().state().census.selections {
        check(cancel)?;
        multiplicity = multiplicity
            .checked_add(row.multiplicity)
            .ok_or_else(|| AppError::limit("selection multiplicity overflow"))?;
        selections
            .record(&json!({"owner":row.owner,"batch":row.batch,
            "baseline":row.baseline,"selected":row.selected,"multiplicity":row.multiplicity}))
            .map_err(storage_error)?;
    }
    if terminals.records != summary.terminals || zeros.records != summary.zeros {
        return Err(AppError::internal_invariant(
            "streamed raw census counters differ",
        ));
    }
    let result = json!({"complete_source_classified_stream":true,
        "terminals":terminals.finish()?,"zeros":zeros.finish()?,"selections":selections.finish()?,
        "selection_multiplicity":multiplicity,"pages_read":summary.pages_read,
        "bytes_read":summary.bytes_read,"page_cache_hits":summary.page_cache_hits,
        "page_evictions":summary.page_evictions,"peak_payload_bytes":summary.peak_payload_bytes,
        "decode_transient_allowance_bytes":summary.decode_transient_allowance_bytes,
        "hashes_are_receipts_not_set_equality_proofs":true});
    check(cancel)?;
    Ok(result)
}

/// Test helper: driver and tiny native tests take exactly the same Session path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn drive<const N: usize>(
    source: &OwnerDomainWalkRequest,
    policy: Policy,
    mode: Mode,
    checkpoint: &Path,
    stop_after: Option<u64>,
    output: &Path,
    budget: &OutputBudget,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Value, AppError> {
    let started = Instant::now();
    let (_, arity, _) = Selection::parse(&source.matching.selection_json)?;
    let parsed = finite_scope::parse(
        source,
        arity,
        policy.max_total_rows,
        policy.max_source_bytes,
    )?;
    let mut session = match mode {
        Mode::Create => Session::<N>::create(source, policy, checkpoint, cancel, observer)?,
        Mode::Resume => Session::<N>::resume(source, policy, checkpoint, cancel, observer)?,
    };
    let prepared_seconds = started.elapsed().as_secs_f64();
    let initial = progress_json(&session.progress()?);
    let rows = parsed
        .rows
        .iter()
        .zip(&session.sources)
        .zip(&session.cursors)
        .enumerate()
        .map(|(ordinal, ((row, windows), cursor))| {
            json!({"ordinal":ordinal,"id":row.query.id,
            "source":row.source,"auxiliary":row.query.auxiliary,
            "role_declared":row.query.role_declared,"owner":row.query.owner,
            "lower":row.query.lower,"upper":row.query.upper,"rank":row.query.rank,
            "power_bounds":{"max_positive_power":row.query.powers.max_positive_power,
                "min_power_difference":row.query.powers.min_power_difference,
                "max_power_difference":row.query.powers.max_power_difference},
            "native_occurrences":windows.total_count().to_string(),
            "initially_exhausted":cursor.is_exhausted(),
            "origin":session.scope.rows[ordinal].origin,
            "exact_domain":hex(session.scope.rows[ordinal].exact_domain),
            "window_policy_binding":hex(windows.source_policy_binding())})
        })
        .collect::<Vec<_>>();
    let mut committed = 0u64;
    let mut failure = None;
    let run_started = Instant::now();
    while !session.progress()?.all_rows_exhausted && stop_after.is_none_or(|stop| committed < stop)
    {
        let step_started = Instant::now();
        match session.step(cancel, observer) {
            Ok(step) => {
                if let Some(window) = step.window {
                    committed =
                        committed
                            .checked_add(u64::try_from(window.windows_count).map_err(|_| {
                                AppError::limit("driver committed group window count")
                            })?)
                            .ok_or_else(|| AppError::limit("driver committed window counter"))?;
                }
                observer(json!({"event":"finite_backend_driver_committed",
                    "step_seconds":step_started.elapsed().as_secs_f64(),
                    "committed_this_process":committed,"progress":progress_json(&step.progress)}));
            }
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    if failure.is_none() {
        failure = check(cancel).err();
    }
    let progress = session.progress()?;
    // A requested boundary stop is not a completed diagnostic, even if that
    // exact boundary also exhausted the scope. Cold resume can stream it later.
    let reached_stop = stop_after.is_some_and(|stop| committed >= stop);
    let run_seconds = run_started.elapsed().as_secs_f64();
    let census_started = Instant::now();
    let census = if failure.is_none() && progress.all_rows_exhausted && !reached_stop {
        match stream_census(&session, output, budget, cancel) {
            Ok(value) => Some(value),
            Err(error) => {
                failure = Some(error);
                None
            }
        }
    } else {
        None
    };
    let census_seconds = census_started.elapsed().as_secs_f64();
    let cursors = session
        .cursors
        .iter()
        .map(|cursor| {
            json!({"exhausted":cursor.is_exhausted(),
        "input_occurrences":cursor.emitted().to_string(),"completed_windows":cursor.windows()})
        })
        .collect::<Vec<_>>();
    // Include session/read caches/reducer teardown in whole-driver time.
    drop(session);
    if failure.is_none() {
        failure = check(cancel).err();
    }
    let status = if failure.is_some() {
        "incomplete"
    } else if reached_stop {
        "paused"
    } else if progress.all_rows_exhausted {
        "complete"
    } else {
        "paused"
    };
    Ok(
        json!({"schema":"rustred.durable-finite-diagnostic.v1","status":status,
        "complete":status=="complete","publication_authority":false,
        "checkpoint_assurance":"trusted_generated","historical_replay":false,
        "mode":mode,"process_id":std::process::id(),"physical_arity":arity,"storage_capacity":N,
        "scope_request_binding":parsed.request_binding,"sources":parsed.sources,"rows":rows,
        "scope_request_binding_is_execution_identity":false,
        "selection_blake3":blake3::hash(source.matching.selection_json.as_bytes()).to_hex().to_string(),
        "initial_progress":initial,"progress":progress_json(&progress),"cursors":cursors,
        "committed_windows_this_process":committed,"stop_after_committed_windows":stop_after,
        "prepared_and_exact_scope_seconds":prepared_seconds,"steps_seconds":run_seconds,
        "final_census_seconds":census_seconds,"including_teardown_seconds":started.elapsed().as_secs_f64(),
        "census":census,"cancelled":cancel.load(Ordering::Acquire),
        "error":failure.as_ref().map(ToString::to_string),
        "error_kind":failure.as_ref().map(|e|format!("{:?}",e.kind()))}),
    )
}

fn read(path: &Path) -> Result<Vec<u8>, AppError> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(execution_error)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(execution_error)?;
    if bytes.len() > 1024 * 1024 {
        return Err(AppError::limit("driver steering exceeds 1 MiB"));
    }
    Ok(bytes)
}

#[test]
#[cfg(all(feature = "cli", unix))]
#[ignore = "manual durable finite diagnostic; external resource supervisor required"]
fn input_controlled_durable_finite_demand() -> Result<(), AppError> {
    if std::env::var("RUSTRED_RUN_DURABLE_FINITE_DEMAND").as_deref() != Ok("1") {
        return Err(AppError::input(
            "explicit durable diagnostic opt-in required",
        ));
    }
    let path = |name| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .ok_or_else(|| AppError::input(format!("explicit {name} required")))
    };
    let command = path("RUSTRED_DURABLE_FINITE_COMMAND")?;
    let steering_path = path("RUSTRED_DURABLE_FINITE_POLICY")?;
    let output = path("RUSTRED_DURABLE_FINITE_OUTPUT")?;
    let command_bytes = read(&command)?;
    let policy_bytes = read(&steering_path)?;
    let steering: Steering =
        serde_json::from_slice(&policy_bytes).map_err(|e| AppError::input(e.to_string()))?;
    if !steering.checkpoint.is_absolute()
        || !output.is_absolute()
        || output.starts_with(&steering.checkpoint)
        || steering.checkpoint.starts_with(&output)
    {
        return Err(AppError::input(
            "explicit disjoint absolute output/checkpoint paths required",
        ));
    }
    std::fs::create_dir(&output).map_err(execution_error)?;
    let started = Instant::now();
    let cancel = Arc::new(AtomicBool::new(false));
    let term = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&cancel))
        .map_err(execution_error)?;
    let interrupt = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&cancel))
        .map_err(execution_error)?;
    let budget = OutputBudget::new(steering.max_output_bytes);
    let mut status = "incomplete".to_string();
    let outcome = (|| -> Result<(), AppError> {
        let args: Vec<String> =
            serde_json::from_slice(&command_bytes).map_err(|e| AppError::input(e.to_string()))?;
        let source =
            crate::cli::walk_request_from_argv(args.into_iter().map(OsString::from).collect())
                .map_err(AppError::input)?;
        let (_, arity, _) = Selection::parse(&source.matching.selection_json)?;
        crate::ensure_runtime_arity(arity)?;
        macro_rules! dispatch { ($($n:literal),*) => { match rustred::campaign_storage_arity(arity) {
            $($n=>drive::<$n>(&source,steering.policy(),steering.mode,&steering.checkpoint,
                steering.stop_after_committed_windows,&output,&budget,&cancel,&|event|eprintln!("{event}")),)*
            _=>Err(AppError::input("durable driver arity is not compiled")),
        }} }
        let mut result = rustred::with_app_runtime_arities!(dispatch)?;
        result["policy"] = serde_json::to_value(&steering).map_err(execution_error)?;
        budget.document(&output.join("result.json"), &result)?;
        if cancel.load(Ordering::Acquire) || result["status"] == "incomplete" {
            return Err(AppError::execution(
                "durable finite diagnostic incomplete; see result",
            ));
        }
        status = result["status"].as_str().unwrap().to_string();
        Ok(())
    })();
    let outcome = outcome.and_then(|()| check(&cancel));
    if outcome.is_err() {
        status = "incomplete".into();
    }
    let receipt = json!({"schema":"rustred.durable-finite-driver.v1","status":status,
        "command_blake3":blake3::hash(&command_bytes).to_hex().to_string(),
        "policy_blake3":blake3::hash(&policy_bytes).to_hex().to_string(),
        "driver_source_blake3":blake3::hash(include_bytes!("diagnostic.rs")).to_hex().to_string(),
        "checkpoint":steering.checkpoint,"process_id":std::process::id(),
        "rayon_threads":rayon::current_num_threads(),"cancelled":cancel.load(Ordering::Acquire),
        "error":outcome.as_ref().err().map(ToString::to_string),
        "evidence_bytes":budget.used.get(),"elapsed_seconds":started.elapsed().as_secs_f64()});
    let receipt_result =
        OutputBudget::new(1024 * 1024).document(&output.join("receipt.json"), &receipt);
    signal_hook::low_level::unregister(term);
    signal_hook::low_level::unregister(interrupt);
    receipt_result?;
    outcome
}

#[cfg(test)]
mod tests;
