//! One compute pool for independent families, preserving ordinary generation,
//! per-sector checkpoints and exact candidate output.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustred::solver::SectorExecutor;
use serde::{Deserialize, Serialize};

use super::{
    args::{self, Command, FamilyCandidatesArgs, StreamPath},
    candidates,
    error::CliError,
    io::{read_input, write_output},
    progress::FamilyGenerationTelemetry,
};
use crate::{FamilyCandidatesRequest, FamilyCloseProgress};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema: String,
    workers: usize,
    max_active_sectors: Option<usize>,
    family_slots: Option<usize>,
    jobs: Vec<Job>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    id: String,
    arguments: Vec<String>,
}
struct Prepared {
    id: String,
    arguments: FamilyCandidatesArgs,
    request: FamilyCandidatesRequest,
}

#[derive(Clone, Serialize)]
struct Parent {
    id: String,
    state: &'static str,
    error: Option<String>,
}
struct State {
    parents: Vec<Parent>,
    state: &'static str,
}

struct Signals(Vec<signal_hook::SigId>);
impl Drop for Signals {
    fn drop(&mut self) {
        for id in &self.0 {
            signal_hook::low_level::unregister(*id);
        }
    }
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Publish explicit parent outcomes and separate performance observations.
/// A failed parent authorizes supervisor shutdown; counters never establish
/// checkpoint validity, successful generation, or mathematical closure.
fn publish(path: &Path, state: &Mutex<State>, pool: &SectorExecutor) {
    let state = state.lock().unwrap();
    let pool = pool.shared_snapshot().map(|s| {
        serde_json::json!({
        "workers":s.workers,"max_active_sectors":s.max_active_sectors,"admitted":s.admitted,
        "running":s.running,"waiting_for_worker":s.waiting_for_worker,
        "completed":s.completed,"peak_running":s.peak_running})
    });
    let value = serde_json::json!({"schema":"rustred.shared-generation-progress.v1",
        "pid":std::process::id(),"timestamp_unix_seconds":now(),"state":state.state,
        "pool":pool,"parents":state.parents,"family_closure_claim":false});
    if let Ok(bytes) = serde_json::to_vec(&value) {
        let _ = crate::application::atomic_file::write_file_atomically(path, &bytes, true);
    }
}

pub(super) fn run(arguments: Vec<OsString>) -> Result<(), CliError> {
    let mut config = None;
    let mut progress = None;
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        let option = option
            .into_string()
            .map_err(args::ArgError::NonUtf8Option)?;
        let slot = match option.as_str() {
            "--config" => &mut config,
            "--progress-json" => &mut progress,
            _ => return Err(args::ArgError::UnknownOption(option).into()),
        };
        if slot.is_some() {
            return Err(CliError::Input(format!("duplicate {option}")));
        }
        *slot = Some(PathBuf::from(arguments.next().ok_or_else(|| {
            CliError::Input(format!("missing value for {option}"))
        })?));
    }
    let config = config.ok_or_else(|| CliError::Input("missing --config".into()))?;
    let progress = progress.ok_or_else(|| CliError::Input("missing --progress-json".into()))?;
    let plan: Config = serde_json::from_str(&read_input(&StreamPath::File(config.clone()))?)
        .map_err(|e| CliError::Input(format!("invalid batch configuration: {e}")))?;
    if plan.schema != "rustred.family-candidates-batch.v1"
        || plan.workers == 0
        || plan.jobs.is_empty()
        || plan.family_slots == Some(0)
        || plan.max_active_sectors == Some(0)
    {
        return Err(CliError::Input(
            "invalid shared-generation schema, inventory or worker/sector budget".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut destinations = vec![candidates::resolved_location(&progress)?];
    let mut inputs = vec![candidates::resolved_location(&config)?];
    let mut jobs = Vec::new();
    for job in plan.jobs {
        if job.id.is_empty() || !ids.insert(job.id.clone()) {
            return Err(CliError::Input(
                "batch parent IDs must be nonempty and unique".into(),
            ));
        }
        let Command::FamilyCandidates(arguments) = args::parse_args(
            [
                OsString::from("rustred"),
                OsString::from("family-candidates"),
            ]
            .into_iter()
            .chain(job.arguments.into_iter().map(OsString::from)),
        )?
        else {
            unreachable!()
        };
        if arguments.n_cores != plan.workers {
            return Err(CliError::Input(
                "every parent must use the shared worker budget".into(),
            ));
        }
        if !matches!(arguments.input, StreamPath::File(_))
            || !matches!(arguments.output, StreamPath::File(_))
        {
            return Err(CliError::Input(
                "batch inputs and outputs must name files".into(),
            ));
        }
        for stream in [&arguments.input].into_iter() {
            if let StreamPath::File(path) = stream {
                inputs.push(candidates::resolved_location(path)?);
            }
        }
        for path in arguments
            .discovery_strategy
            .iter()
            .chain(arguments.integral_order.iter())
        {
            inputs.push(candidates::resolved_location(path)?);
        }
        for stream in [&arguments.output]
            .into_iter()
            .chain(arguments.report_output.iter())
        {
            if let StreamPath::File(path) = stream {
                destinations.push(candidates::resolved_location(path)?);
            }
        }
        for path in arguments
            .progress_json
            .iter()
            .chain(arguments.checkpoint.as_ref().map(|c| &c.directory))
        {
            destinations.push(candidates::resolved_location(path)?);
        }
        let request = candidates::prepare(&arguments)?;
        jobs.push(Prepared {
            id: job.id,
            arguments,
            request,
        });
    }
    for (i, path) in destinations.iter().enumerate() {
        if destinations[..i]
            .iter()
            .any(|other| path.starts_with(other) || other.starts_with(path))
            || inputs
                .iter()
                .any(|input| input.starts_with(path) || path == input)
        {
            return Err(CliError::Input("batch outputs, telemetry and checkpoint directories must be disjoint from one another and from inputs".into()));
        }
    }
    let cancellation = Arc::new(AtomicBool::new(false));
    let mut signals = Signals(Vec::new());
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signals.0.push(
            signal_hook::flag::register(signal, Arc::clone(&cancellation))
                .map_err(|e| CliError::InputIo(e.to_string()))?,
        );
    }
    let pool = Arc::new(
        SectorExecutor::new(plan.workers)
            .and_then(|pool| pool.with_shared_scheduling(plan.max_active_sectors))
            .map_err(|e| CliError::Input(e.to_string()))?,
    );
    let state = Arc::new(Mutex::new(State {
        state: "running",
        parents: jobs
            .iter()
            .map(|job| Parent {
                id: job.id.clone(),
                state: "pending",
                error: None,
            })
            .collect(),
    }));
    publish(&progress, &state, &pool);
    let (finished, stop) = std::sync::mpsc::channel();
    let (published, publication_done) = std::sync::mpsc::channel();
    let publisher_state = Arc::clone(&state);
    let publisher_pool = Arc::clone(&pool);
    let publisher_path = progress.clone();
    let publisher = std::thread::Builder::new()
        .name("rustred-shared-progress".into())
        .spawn(move || {
            loop {
                match stop.recv_timeout(Duration::from_secs(1)) {
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        publish(&publisher_path, &publisher_state, &publisher_pool);
                        let _ = published.send(());
                        break;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
                publish(&publisher_path, &publisher_state, &publisher_pool);
            }
        })
        .map_err(|e| CliError::InputIo(e.to_string()))?;
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..plan.family_slots.unwrap_or(jobs.len()).min(jobs.len()) {
            let (jobs, next, pool, state, cancellation) =
                (&jobs, &next, &pool, &state, &cancellation);
            scope.spawn(move || {
                loop {
                    if cancellation.load(Ordering::Acquire) {
                        break;
                    }
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(index) else {
                        break;
                    };
                    state.lock().unwrap().parents[index].state = "preparing";
                    let mut executor = pool.as_ref().clone();
                    executor = executor.with_cancellation(Arc::clone(cancellation));
                    let telemetry = Mutex::new(FamilyGenerationTelemetry::new(
                        job.arguments.progress_json.clone(),
                    ));
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let result = crate::family_candidates_on_executor(
                            job.request.clone(),
                            &executor,
                            Arc::clone(cancellation),
                            |event| {
                                if matches!(event, FamilyCloseProgress::Prepared { .. }) {
                                    state.lock().unwrap().parents[index].state = "running";
                                }
                                if let Ok(mut telemetry) = telemetry.lock() {
                                    telemetry.observe(event);
                                }
                            },
                        )
                        .map_err(CliError::from)?;
                        write_output(&job.arguments.output, result.bundle(), job.arguments.force)?;
                        if let Some(report) = &job.arguments.report_output {
                            write_output(report, result.to_toml().as_bytes(), job.arguments.force)?;
                        }
                        Ok::<_, CliError>(())
                    }))
                    .unwrap_or_else(|_| Err(CliError::Input("parent generation panicked".into())));
                    if let Ok(mut telemetry) = telemetry.lock() {
                        telemetry.finish(result.is_ok());
                    }
                    let mut state = state.lock().unwrap();
                    let parent = &mut state.parents[index];
                    match result {
                        Ok(()) => parent.state = "completed",
                        Err(error) => {
                            let cancelled = matches!(&error, CliError::Application(error)
                            if error.kind() == crate::AppErrorKind::Cancelled);
                            if cancelled {
                                parent.state = "interrupted";
                            } else {
                                parent.state = "failed";
                                parent.error = Some(error.to_string());
                                cancellation.store(true, Ordering::Release);
                            }
                        }
                    }
                }
            });
        }
    });
    {
        let mut state = state.lock().unwrap();
        state.state = if state.parents.iter().all(|p| p.state == "completed") {
            "completed"
        } else if state.parents.iter().any(|p| p.state == "failed") {
            "failed"
        } else {
            "stopped"
        };
    }
    let _ = finished.send(());
    // A blocked presentation filesystem cannot hold completed generation open.
    if publication_done
        .recv_timeout(Duration::from_millis(500))
        .is_ok()
    {
        let _ = publisher.join();
    }
    let state = state.lock().unwrap();
    if state.state != "completed" {
        let failed = state.parents.iter().find(|parent| parent.error.is_some());
        return Err(CliError::Input(failed.map_or_else(
            || "shared generation interrupted; completed sectors retained".into(),
            |p| {
                format!(
                    "shared generation {}: {}",
                    p.id,
                    p.error.as_deref().unwrap_or("unknown error")
                )
            },
        )));
    }
    Ok(())
}
