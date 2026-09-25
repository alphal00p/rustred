use super::{
    CliError, artifact, bad,
    checkpoint::{self, MasterState, Snapshot},
    config::Config,
    events::{EventLog, NativeTail},
    io_error, monitor, now,
    resources::{self, MemoryAction},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

struct Active {
    id: usize,
    slot: usize,
    child: Child,
    start_ticks: u64,
    cpus: Vec<usize>,
    directory: PathBuf,
    tail: NativeTail,
    progress: Value,
    cpu_seconds: f64,
    rss_bytes: u64,
    busy_cores: f64,
    started: Instant,
    previous_completed: Option<(u64, f64)>,
    recent_domains_per_second: Option<f64>,
}
impl Active {
    fn stop(&self) -> Result<(), CliError> {
        let path = self.directory.join("stop-request.json");
        match OpenOptions::new().create_new(true).write(true).open(path) {
            Ok(mut file) => file
                .write_all(b"{\"reason\":\"supervisor_cooperative_pause\"}\n")
                .map_err(io_error),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(io_error(error)),
        }
    }
    fn poll(&mut self) -> Result<(), CliError> {
        self.tail.poll(&self.directory.join("events.jsonl"))?;
        let native = self
            .tail
            .latest
            .get("progress")
            .filter(|v| v.is_object())
            .unwrap_or(&self.tail.latest);
        let counters = native
            .get("snapshot")
            .filter(|v| v.is_object())
            .unwrap_or(native);
        if let Some(fields) = counters.as_object() {
            for (key, value) in fields {
                self.progress[key] = value.clone();
            }
        }
        if let Some(completed) = self.progress["completed_nodes"].as_u64() {
            self.recent_domains_per_second = recent_rate(
                &mut self.previous_completed,
                completed,
                self.started.elapsed().as_secs_f64(),
            );
        }
        Ok(())
    }
}

/// Any supervisor error first requests a durable pause and reaps its children.
/// There is deliberately no time-based kill fallback. RAM emergencies alone
/// use the identity-checked kill path below.
struct Children(Vec<Active>, u64, u64);
impl Drop for Children {
    fn drop(&mut self) {
        for job in &self.0 {
            let _ = job.stop();
        }
        while !self.0.is_empty() {
            let groups = self.0.iter().map(|j| j.child.id()).collect::<BTreeSet<_>>();
            if let (Ok(samples), Ok(available)) = (
                resources::group_samples(&groups),
                resources::host_available(),
            ) {
                let rss = samples.iter().map(|s| s.rss_bytes).sum();
                if resources::memory_action(rss, available, self.1, self.2)
                    == MemoryAction::Emergency
                {
                    for job in &self.0 {
                        let _ = resources::emergency_kill(job.child.id(), job.start_ticks);
                    }
                }
            }
            self.0.retain_mut(|job| match job.child.try_wait() {
                Ok(Some(_)) => false,
                Ok(None) => true,
                Err(_) => {
                    let _ = job.child.wait();
                    false
                }
            });
            if !self.0.is_empty() {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}
struct Signals(Vec<signal_hook::SigId>);
impl Signals {
    fn register(flag: Arc<AtomicBool>) -> Result<Self, CliError> {
        let mut result = Self(vec![]);
        for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
            result
                .0
                .push(signal_hook::flag::register(signal, Arc::clone(&flag)).map_err(io_error)?);
        }
        Ok(result)
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        for id in &self.0 {
            signal_hook::low_level::unregister(*id);
        }
    }
}

pub(super) fn run(
    config_path: Option<&Path>,
    directory: &Path,
    resume: bool,
) -> Result<(), CliError> {
    if !resume {
        if config_path.is_none() {
            return Err(bad("initial launch requires --config PATH"));
        }
        // Validate before reserving a campaign directory.
        Config::read(config_path.unwrap())?;
        fs::create_dir(directory).map_err(|e| {
            bad(format!(
                "new campaign directory {}: {e}",
                directory.display()
            ))
        })?;
    }
    if fs::symlink_metadata(directory)
        .map_err(io_error)?
        .file_type()
        .is_symlink()
    {
        return Err(bad("campaign directory cannot be a symlink"));
    }
    let directory = fs::canonicalize(directory).map_err(io_error)?;
    let _lock = checkpoint::acquire_lock(&directory)?;
    let snapshot = if resume {
        let snapshot: Snapshot =
            checkpoint::read_json(&directory.join("snapshot.json"), 16 * 1024 * 1024)?;
        snapshot.verify(&directory)?;
        if let Some(path) = config_path {
            if Config::read(path)? != snapshot.config {
                return Err(bad("configuration differs from frozen campaign policy"));
            }
            for source in &snapshot.source_files {
                source.verify(Path::new(""))?;
            }
        }
        snapshot
    } else {
        artifact::prepare(Config::read(config_path.unwrap())?, &directory)?
    };
    let config = &snapshot.config;
    let allowed = resources::allowed_cpus()?;
    if config.cpus.iter().any(|cpu| !allowed.contains(cpu)) {
        return Err(bad(
            "configured CPU set is not a subset of the supervisor's allowed affinity",
        ));
    }
    let mut state = if resume {
        checkpoint::read_json(&directory.join("state.json"), 16 * 1024 * 1024)?
    } else {
        MasterState::new(&snapshot.shards)
    };
    validate_resume(&directory, &snapshot, &mut state)?;
    if resume && directory.join("stop-request.json").exists() {
        fs::rename(
            directory.join("stop-request.json"),
            directory.join(format!(
                "stop-request-acknowledged-{}-{}.json",
                std::process::id(),
                now()
            )),
        )
        .map_err(io_error)?;
    }
    state.state = "running".into();
    state.save(&directory)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let _signals = Signals::register(Arc::clone(&cancel))?;
    let mut log = EventLog::open(&directory.join("events.jsonl"))?;
    let mut presenter = monitor::Presenter::new();
    let resume_command = format!(
        "{} campaign shards --directory {} --resume",
        quote(&directory.join("bin/rustred")),
        quote(&directory)
    );
    log.emit(&json!({"event":"supervisor_started","timestamp_unix_seconds":now(),"resume":resume,"resume_command":resume_command,
        "shards":snapshot.shards.len(),"workers_budget":config.total_workers,"jobs_limit":config.jobs,
        "shared_descendant_coverage":false,"family_closure_claim":false}))?;
    let started = Instant::now();
    let mut sampled_at = Instant::now();
    let mut previous_ticks = BTreeMap::<(u32, u64), u64>::new();
    let mut children = Children(vec![], config.max_memory_bytes, config.host_reserve_bytes);
    let mut stop_reason: Option<String> = None;
    let mut failure = false;
    let mut peak_rss = 0;
    let mut cumulative_cpu = 0.0;
    loop {
        if cancel.load(Ordering::Relaxed) || directory.join("stop-request.json").exists() {
            request_stop(&mut stop_reason, "operator_request", &children, &mut log)?;
        }
        let available = resources::host_available()?;
        if available <= config.host_reserve_bytes {
            request_stop(&mut stop_reason, "host_memory_reserve", &children, &mut log)?;
        }
        if stop_reason.is_none() {
            while children.0.len() < config.jobs {
                let Some(id) = state.jobs.iter().position(|job| job.state == "queued") else {
                    break;
                };
                let slot = (0..config.jobs)
                    .find(|slot| children.0.iter().all(|j| j.slot != *slot))
                    .unwrap();
                let attempt = state.jobs[id].attempt + 1;
                let active = launch(&directory, &snapshot, id, slot, attempt, Some(&_lock))?;
                state.jobs[id].state = "running".into();
                state.jobs[id].attempt = attempt;
                state.jobs[id].pid = Some(active.child.id());
                state.jobs[id].start_ticks = Some(active.start_ticks);
                // Register ownership before any further fallible filesystem I/O.
                children.0.push(active);
                state.save(&directory)?;
                let active = children.0.last().unwrap();
                log.emit(&json!({"event":"shard_started","timestamp_unix_seconds":now(),"id":id,"attempt":attempt,"pid":active.child.id(),"start_ticks":active.start_ticks,"cpus":active.cpus,"workers":config.workers_per_job}))?;
            }
        }
        let groups = children
            .0
            .iter()
            .map(|j| j.child.id())
            .collect::<BTreeSet<_>>();
        let samples = resources::group_samples(&groups)?;
        let interval = sampled_at.elapsed().as_secs_f64().max(0.001);
        sampled_at = Instant::now();
        let mut deltas = BTreeMap::<u32, f64>::new();
        let mut group_rss = BTreeMap::<u32, u64>::new();
        let mut rss = 0;
        let mut current_ticks = BTreeMap::new();
        for row in &samples {
            rss += row.rss_bytes;
            let key = (row.pid, row.start_ticks);
            let delta = previous_ticks
                .get(&key)
                .map_or(0, |previous| row.cpu_ticks.saturating_sub(*previous))
                as f64
                / resources::ticks_per_second();
            *deltas.entry(row.group).or_default() += delta;
            *group_rss.entry(row.group).or_default() += row.rss_bytes;
            current_ticks.insert(key, row.cpu_ticks);
        }
        previous_ticks = current_ticks;
        peak_rss = peak_rss.max(rss);
        let delta_cpu: f64 = deltas.values().sum();
        cumulative_cpu += delta_cpu;
        for job in &mut children.0 {
            let delta = deltas.get(&job.child.id()).copied().unwrap_or(0.0);
            job.cpu_seconds += delta;
            job.busy_cores = delta / interval;
            job.rss_bytes = group_rss.get(&job.child.id()).copied().unwrap_or(0);
            job.poll()?;
        }
        match resources::memory_action(
            rss,
            available,
            config.max_memory_bytes,
            config.host_reserve_bytes,
        ) {
            MemoryAction::Continue => {}
            MemoryAction::Save => request_stop(
                &mut stop_reason,
                "aggregate_memory_soft_limit",
                &children,
                &mut log,
            )?,
            MemoryAction::Emergency => {
                let stop_result =
                    request_stop(&mut stop_reason, "memory_emergency", &children, &mut log);
                failure = true;
                let mut kill_error = None;
                for job in &children.0 {
                    if let Err(error) = resources::emergency_kill(job.child.id(), job.start_ticks) {
                        kill_error.get_or_insert(error);
                    }
                }
                stop_result?;
                if let Some(error) = kill_error {
                    return Err(error);
                }
            }
        }
        let mut index = 0;
        while index < children.0.len() {
            let Some(exit) = children.0[index].child.try_wait().map_err(io_error)? else {
                index += 1;
                continue;
            };
            let mut job = children.0.remove(index);
            job.poll()?;
            while job.tail.backlogged {
                job.poll()?;
            }
            let outcome = finish(&directory, &snapshot, &mut state, &job, exit.code());
            if let Err(error) = outcome {
                state.jobs[job.id].state = "failed".into();
                state.jobs[job.id].last_error = Some(error.to_string());
                failure = true;
                request_stop(&mut stop_reason, "shard_failed", &children, &mut log)?;
            }
            if state.jobs[job.id].state == "paused" {
                request_stop(&mut stop_reason, "child_paused", &children, &mut log)?;
            }
            state.jobs[job.id].pid = None;
            state.jobs[job.id].start_ticks = None;
            log.emit(&json!({"event":"shard_finished","timestamp_unix_seconds":now(),"id":job.id,"state":state.jobs[job.id].state,"exit_code":exit.code(),"error":state.jobs[job.id].last_error,"native_tail":job.tail.diagnostics()}))?;
            state.save(&directory)?;
        }
        let all_complete = state.jobs.iter().all(|job| job.state == "completed");
        if all_complete {
            let receipts = state
                .jobs
                .iter()
                .map(|job| {
                    checkpoint::read_json(
                        &directory.join(&job.receipt.as_ref().unwrap().path),
                        1024 * 1024,
                    )
                })
                .collect::<Result<Vec<Value>, _>>()?;
            artifact::complete(&directory, &snapshot, receipts)?;
        }
        let terminal = children.0.is_empty() && (all_complete || stop_reason.is_some());
        let phase = if all_complete {
            "completed"
        } else if terminal && failure {
            "failed"
        } else if terminal {
            "paused"
        } else if stop_reason.is_some() {
            "stopping"
        } else {
            "running"
        };
        let status = status(
            &directory,
            &snapshot,
            &state,
            &children,
            phase,
            &resume_command,
            started.elapsed().as_secs_f64(),
            rss,
            peak_rss,
            cumulative_cpu,
            delta_cpu / interval,
            available,
            stop_reason.as_deref(),
        );
        checkpoint::write_json(&directory.join("status.json"), &status, true)?;
        log.emit(&json!({"event":"status","status":status}))?;
        // Terminal presentation is not part of the mathematical run. A closed
        // stderr must not interrupt native computation or checkpoint saving.
        let _ = presenter.update(&status);
        if terminal {
            state.state = phase.into();
            state.save(&directory)?;
            if all_complete {
                return Ok(());
            }
            eprintln!("Campaign {phase}; resume with: {resume_command}");
            return Err(bad(format!(
                "campaign {phase}: {}; resume with {resume_command}",
                stop_reason.as_deref().unwrap_or("unfinished")
            )));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn validate_resume(
    directory: &Path,
    snapshot: &Snapshot,
    state: &mut MasterState,
) -> Result<(), CliError> {
    if state.schema != "rustred.independent-root-state.v1"
        || state.jobs.len() != snapshot.shards.len()
    {
        return Err(bad("invalid master state schema or shard count"));
    }
    for (id, job) in state.jobs.iter_mut().enumerate() {
        if job.id != id || snapshot.shards[id].id != id {
            return Err(bad("master state shard IDs differ from immutable plan"));
        }
        if job.pid.is_some() != job.start_ticks.is_some() {
            return Err(bad("incomplete saved process identity"));
        }
        if let (Some(pid), Some(start)) = (job.pid, job.start_ticks) {
            if resources::same_process(pid, start) {
                return Err(bad(format!(
                    "shard {id} is still alive (PID {pid}); refusing a second instance"
                )));
            }
        }
        // Recover the durable receipt if the supervisor crashed between receipt
        // installation and updating the master state. A journal-only "finished"
        // event is insufficient: the lost child exit status is not guessed.
        let receipt_path = directory.join(format!("jobs/{id:04}/completion.json"));
        if job.state != "completed" && receipt_path.exists() {
            let mut identity = checkpoint::FileIdentity::record(&receipt_path)?;
            identity.path = receipt_path.strip_prefix(directory).unwrap().to_owned();
            job.receipt = Some(identity);
            job.state = "completed".into();
        }
        if job.state == "completed" {
            let receipt = job
                .receipt
                .as_ref()
                .ok_or_else(|| bad("completed shard is missing its durable receipt"))?;
            receipt.verify(directory)?;
            let value: Value = checkpoint::read_json(&directory.join(&receipt.path), 1024 * 1024)?;
            if value["id"].as_u64() != Some(id as u64) || value["exit_code"] != 0 {
                return Err(bad(
                    "invalid completed shard receipt identity or exit status",
                ));
            }
            if value["owners"] != json!(snapshot.shards[id].owners)
                || value["query_indices"] != json!(snapshot.shards[id].query_indices)
                || value["executable_blake3"]
                    != json!(
                        snapshot
                            .files
                            .iter()
                            .find(|f| f.path == Path::new("bin/rustred"))
                            .map(|f| &f.blake3)
                    )
            {
                return Err(bad(
                    "completed shard receipt differs from the frozen plan or executable",
                ));
            }
            checkpoint::validate_completion(&value["native_completion"])?;
            job.native_summary = Some(value["native_completion"].clone());
            job.pid = None;
            job.start_ticks = None;
        } else if matches!(
            job.state.as_str(),
            "queued" | "running" | "paused" | "failed"
        ) {
            job.state = "queued".into();
            job.pid = None;
            job.start_ticks = None;
            job.last_error = None;
        } else {
            return Err(bad("unknown shard state"));
        }
    }
    Ok(())
}

fn launch(
    directory: &Path,
    snapshot: &Snapshot,
    id: usize,
    slot: usize,
    attempt: usize,
    master_lock: Option<&File>,
) -> Result<Active, CliError> {
    let config = &snapshot.config;
    let jobdir = directory.join(format!("jobs/{id:04}"));
    let run = jobdir.join(format!("attempt-{attempt:04}"));
    fs::create_dir(&run).map_err(io_error)?;
    let checkpoint = jobdir.join("checkpoint");
    let resume = checkpoint.join("latest.json").is_file();
    if !resume
        && checkpoint.exists()
        && fs::read_dir(&checkpoint)
            .map_err(io_error)?
            .next()
            .is_some()
    {
        return Err(bad(format!(
            "shard {id} has an incomplete checkpoint directory without latest.json"
        )));
    }
    let queries = jobdir.join("queries.json");
    let argv = vec![
        "owner-domain-match".into(),
        "--manifest".into(),
        directory.join("artifact/selection.json").into_os_string(),
        "--owner-base".into(),
        directory.join("artifact").into_os_string(),
        "--queries".into(),
        queries.clone().into_os_string(),
        "--output".into(),
        run.join("result.json").into_os_string(),
        "--events".into(),
        run.join("events.jsonl").into_os_string(),
        "--stop-file".into(),
        run.join("stop-request.json").into_os_string(),
        "--follow-successors".into(),
        "--unbounded-work".into(),
        "--no-progress".into(),
        "--workers".into(),
        config.workers_per_job.to_string().into(),
        "--publication-policy".into(),
        config.publication_policy.clone().into(),
        "--max-queries".into(),
        snapshot.shards[id].query_indices.len().to_string().into(),
        "--max-query-bytes".into(),
        fs::metadata(&queries)
            .map_err(io_error)?
            .len()
            .to_string()
            .into(),
        if resume {
            "--resume".into()
        } else {
            "--checkpoint".into()
        },
        checkpoint.into_os_string(),
        "--checkpoint-interval-seconds".into(),
        config.checkpoint_interval_seconds.to_string().into(),
    ];
    let mut argv = argv;
    argv.extend(config.native_options.iter().map(std::ffi::OsString::from));
    let cpus =
        config.cpus[slot * config.workers_per_job..(slot + 1) * config.workers_per_job].to_vec();
    let arguments: Vec<_> = argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    checkpoint::write_json(
        &run.join("launch.json"),
        &json!({"schema":"rustred.independent-root-launch.v1","id":id,"attempt":attempt,"executable":directory.join("bin/rustred"),"arguments":arguments,"cpus":cpus,"resume":resume}),
        false,
    )?;
    let mut command = Command::new(directory.join("bin/rustred"));
    command.args(&argv).stdin(Stdio::null());
    command
        .stdout(File::create(run.join("stdout.log")).map_err(io_error)?)
        .stderr(File::create(run.join("stderr.log")).map_err(io_error)?);
    for name in [
        "RAYON_NUM_THREADS",
        "OMP_NUM_THREADS",
        "OMP_THREAD_LIMIT",
        "OPENBLAS_NUM_THREADS",
        "MKL_NUM_THREADS",
        "BLIS_NUM_THREADS",
        "SYMBOLICA_HIDE_BANNER",
    ] {
        command.env(name, "1");
    }
    resources::configure(&mut command, &cpus, master_lock)?;
    let child = command.spawn().map_err(io_error)?;
    let start_ticks = resources::sample(child.id())
        .map(|sample| sample.start_ticks)
        .unwrap_or(0);
    // /proc still retains an already-exited child until we reap it; a missing
    // identity is a supervision failure, not a licence to signal another PID.
    let active = Active {
        id,
        slot,
        child,
        start_ticks,
        cpus,
        directory: run,
        tail: NativeTail::new(),
        progress: json!({}),
        cpu_seconds: 0.0,
        rss_bytes: 0,
        busy_cores: 0.0,
        started: Instant::now(),
        previous_completed: None,
        recent_domains_per_second: None,
    };
    if start_ticks == 0 {
        let _ = active.stop();
        let mut child = active.child;
        let _ = child.wait();
        return Err(bad("cannot establish child process identity"));
    }
    Ok(active)
}

fn finish(
    directory: &Path,
    snapshot: &Snapshot,
    state: &mut MasterState,
    job: &Active,
    exit: Option<i32>,
) -> Result<(), CliError> {
    let completion = job
        .tail
        .completion
        .as_ref()
        .ok_or_else(|| bad("child exited without a complete bounded native final event"))?;
    state.jobs[job.id].native_summary = Some(completion.clone());
    state.jobs[job.id].saved_checkpoint = job.tail.saved_checkpoint.clone();
    if exit == Some(0) {
        checkpoint::validate_completion(completion)?;
        let output = fs::metadata(job.directory.join("result.json")).map_err(io_error)?;
        if !output.is_file() || output.len() == 0 {
            return Err(bad("successful native child has no completed result file"));
        }
        let receipt = json!({"schema":"rustred.independent-root-shard-receipt.v1","id":job.id,"owners":snapshot.shards[job.id].owners,
            "query_indices":snapshot.shards[job.id].query_indices,"exit_code":0,"native_completion":completion,
            "result_bytes":output.len(),"result_path":job.directory.join("result.json"),"sampled_cpu_seconds":job.cpu_seconds,
            "executable_blake3":snapshot.files.iter().find(|f|f.path==Path::new("bin/rustred")).map(|f|&f.blake3),"finished_unix_seconds":now()});
        let path = directory.join(format!("jobs/{:04}/completion.json", job.id));
        checkpoint::write_json(&path, &receipt, false)?;
        let mut identity = checkpoint::FileIdentity::record(&path)?;
        identity.path = path.strip_prefix(directory).unwrap().to_owned();
        state.jobs[job.id].receipt = Some(identity);
        state.jobs[job.id].state = "completed".into();
    } else if exit == Some(4)
        && completion["status"] == "paused"
        && job
            .tail
            .saved_checkpoint
            .as_ref()
            .is_some_and(|v| v["state"] == "saved")
    {
        let manifest: Value = checkpoint::read_json(
            &directory.join(format!("jobs/{:04}/checkpoint/latest.json", job.id)),
            1024 * 1024,
        )?;
        if manifest["metadata"]["state"] != "saved" {
            return Err(bad("paused child is missing a saved checkpoint manifest"));
        }
        state.jobs[job.id].state = "paused".into();
    } else {
        return Err(bad(format!(
            "shard {} exited {:?} without successful completion or durable pause",
            job.id, exit
        )));
    }
    Ok(())
}

fn request_stop(
    reason: &mut Option<String>,
    requested: &str,
    children: &Children,
    log: &mut EventLog,
) -> Result<(), CliError> {
    if reason.is_none() {
        *reason = Some(requested.into());
        // Signal every owned child before writing the informational journal.
        let mut first = None;
        for job in &children.0 {
            if let Err(error) = job.stop() {
                first.get_or_insert(error);
            }
        }
        if let Some(error) = first {
            return Err(error);
        }
        log.emit(&json!({"event":"cooperative_stop_requested","timestamp_unix_seconds":now(),"reason":requested}))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn status(
    directory: &Path,
    snapshot: &Snapshot,
    state: &MasterState,
    children: &Children,
    phase: &str,
    resume_command: &str,
    elapsed: f64,
    rss: u64,
    peak: u64,
    cpu: f64,
    busy: f64,
    available: u64,
    stop_reason: Option<&str>,
) -> Value {
    let count = |name: &str| state.jobs.iter().filter(|j| j.state == name).count();
    let roots = |name: &str| {
        state
            .jobs
            .iter()
            .filter(|j| j.state == name)
            .map(|j| snapshot.shards[j.id].owners.len())
            .sum::<usize>()
    };
    let mut completed = 0;
    let mut pending = 0;
    let mut frontiers = 0;
    let mut checkpoint_time: Option<f64> = None;
    for job in &state.jobs {
        if children.0.iter().any(|active| active.id == job.id) {
            continue;
        }
        if let Some(native) = &job.native_summary {
            completed += native["completed_nodes"].as_u64().unwrap_or(0);
            pending += native["queued_nodes"].as_u64().unwrap_or(0);
            frontiers += native["frontiers"].as_u64().unwrap_or(0);
        }
        if let Some(time) = job
            .saved_checkpoint
            .as_ref()
            .and_then(|v| v["saved_unix_time"].as_f64())
        {
            checkpoint_time = Some(checkpoint_time.map_or(time, |old| old.min(time)));
        }
    }
    let active:Vec<_>=children.0.iter().map(|job|{
        let native=&job.progress;let saved=job.tail.saved_checkpoint.as_ref();
        let saved_at=saved.and_then(|v|v["saved_unix_time"].as_f64());
        if let Some(time)=saved_at{checkpoint_time=Some(checkpoint_time.map_or(time,|old|old.min(time)));}
        completed+=native["completed_nodes"].as_u64().or_else(||native["completed_domains"].as_u64()).unwrap_or(0);
        pending+=native["queued_nodes"].as_u64().or_else(||native["pending_domains"].as_u64()).unwrap_or(0);
        frontiers+=native["frontiers"].as_u64().unwrap_or(0);
        json!({"id":job.id,"pid":job.child.id(),"start_ticks":job.start_ticks,"owners":snapshot.shards[job.id].owners,"roots":snapshot.shards[job.id].owners.len(),
            "workers":snapshot.config.workers_per_job,"cpus":job.cpus,"state":if stop_reason.is_some(){"stopping"}else{"running"},
            "cpu_seconds":job.cpu_seconds,"rss_bytes":job.rss_bytes,"measured_busy_cores":job.busy_cores,"recent_domains_per_second":job.recent_domains_per_second,
            "completed_domains":native["completed_nodes"].as_u64().or_else(||native["completed_domains"].as_u64()),
            "pending_domains":native["queued_nodes"].as_u64().or_else(||native["pending_domains"].as_u64()),"frontiers":native["frontiers"],
            "checkpoint_status":if job.tail.checkpoint_write.is_some(){"writing"}else if saved.is_some(){"saved"}else{"unavailable"},
            "checkpoint_saved_at_unix_seconds":saved_at,"checkpoint_age_seconds":saved_at.map(|t|(now()-t).max(0.0)),
            "progress_updated_at_unix_seconds":job.tail.progress_updated_at_unix_seconds,"native_tail":job.tail.diagnostics(),"event_tail_backlogged":job.tail.backlogged})
    }).collect();
    let mut result = json!({"schema":"rustred.independent-root-status.v1","state":phase,"timestamp_unix_seconds":now(),"directory":directory,"resume_command":resume_command,
        "workers_budget":snapshot.config.total_workers,"running_workers":children.0.len()*snapshot.config.workers_per_job,"jobs_limit":snapshot.config.jobs,"cpus":snapshot.config.cpus,
        "shards_total":snapshot.shards.len(),"shards_completed":count("completed"),"shards_running":children.0.len(),"shards_queued":count("queued"),"shards_paused":count("paused"),"shards_failed":count("failed"),
        "roots_total":snapshot.root_count,"roots_completed":roots("completed"),"roots_running":roots("running"),"roots_queued":roots("queued"),"roots_paused":roots("paused"),"roots_failed":roots("failed")});
    let counters = json!({
        "rss_bytes":rss,"peak_rss_bytes":peak,"max_memory_bytes":snapshot.config.max_memory_bytes,"measured_busy_cores":busy,"cpu_seconds":cpu,"cpu_accounting":"sampled owned process groups and supervisor; worker reservation is not CPU utilization",
        "elapsed_seconds":elapsed,"completed_domains":completed,"pending_domains":pending,"frontiers":frontiers,"host_available_bytes":available,
        "recent_domains_per_second":if children.0.is_empty(){None}else{children.0.iter().map(|job|job.recent_domains_per_second).collect::<Option<Vec<_>>>().map(|rates|rates.iter().sum::<f64>())},
        "throughput_basis":"current-attempt differences in native completed_nodes; unknown before two observations or after counter regression",
        "errors":state.jobs.iter().filter(|job|job.last_error.is_some()).count(),"last_error":state.jobs.iter().find_map(|job|job.last_error.as_ref()),
        "event_tail_backlogged":children.0.iter().any(|job|job.tail.backlogged)});
    let progress = json!({
        "latest_checkpoint_age_seconds":checkpoint_time.map(|t|(now()-t).max(0.0)),"checkpoint_saved_at_unix_seconds":checkpoint_time,
        "checkpoint_status":if children.0.iter().any(|job|job.tail.checkpoint_write.is_some()){"writing"}else if checkpoint_time.is_some(){"saved"}else{"unavailable"},
        "active":active,"artifact_state":if phase=="completed"{"complete"}else{"prepared"},"artifact_path":directory.join("artifact"),
        "stop_reason":stop_reason,"shared_descendant_coverage":false,"family_closure_claim":false,"recursive_completion_fraction":Value::Null});
    for value in [counters, progress] {
        if let Value::Object(fields) = value {
            result.as_object_mut().unwrap().extend(fields);
        }
    }
    result
}

fn quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn recent_rate(previous: &mut Option<(u64, f64)>, completed: u64, elapsed: f64) -> Option<f64> {
    let old = previous.replace((completed, elapsed));
    let (old_count, old_elapsed) = old?;
    let interval = elapsed - old_elapsed;
    if !interval.is_finite() || interval <= 0.0 {
        return None;
    }
    Some(completed.checked_sub(old_count)? as f64 / interval)
}

#[cfg(test)]
#[path = "supervisor_tests.rs"]
mod tests;
