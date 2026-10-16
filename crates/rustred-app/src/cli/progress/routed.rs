//! One bounded progress slot, lossless checkpoint milestones, and an independent heartbeat.
use super::terminal::{TerminalSession, resident_set_bytes};
use serde_json::{Value, json};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
pub(super) mod finite;

pub(crate) struct RoutedProgress {
    latest: Arc<Mutex<(Instant, Value)>>,
    stop: mpsc::SyncSender<Control>,
    handle: Option<JoinHandle<io::Result<()>>>,
}
enum Control {
    Checkpoint(Value),
    Stop,
}

#[derive(Clone, Copy)]
struct NodeSample {
    time: Instant,
    expanded: u64,
    queued: u64,
    window: Option<(u64, u64)>,
}

impl NodeSample {
    fn rates_since(self, previous: Option<Self>, finite: bool) -> Option<(f64, f64)> {
        let previous = previous?;
        if finite && (self.window.is_none() || self.window != previous.window) {
            return None;
        }
        let seconds = self
            .time
            .duration_since(previous.time)
            .as_secs_f64()
            .max(f64::EPSILON);
        Some((
            self.expanded.saturating_sub(previous.expanded) as f64 / seconds,
            (self.queued as f64 - previous.queued as f64) / seconds,
        ))
    }
}

fn write_record(events: &mut dyn Write, record: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *events, record).map_err(io::Error::other)?;
    events.write_all(b"\n")?;
    events.flush()
}
impl RoutedProgress {
    pub fn start(
        events: Box<dyn Write + Send>,
        tty: bool,
        cancellation: Arc<AtomicBool>,
        stop_file: Option<PathBuf>,
    ) -> Self {
        Self::start_with_mode(events, tty, cancellation, stop_file, None, tty)
    }

    pub fn start_finite(
        events: Box<dyn Write + Send>,
        tty: bool,
        cancellation: Arc<AtomicBool>,
        stop_file: Option<PathBuf>,
        reserved_workers: usize,
        progress_enabled: bool,
    ) -> Self {
        Self::start_with_mode(
            events,
            tty,
            cancellation,
            stop_file,
            Some(reserved_workers),
            progress_enabled,
        )
    }

    fn start_with_mode(
        mut events: Box<dyn Write + Send>,
        tty: bool,
        cancellation: Arc<AtomicBool>,
        stop_file: Option<PathBuf>,
        reserved_workers: Option<usize>,
        progress_enabled: bool,
    ) -> Self {
        let finite_mode = reserved_workers.is_some();
        let initial = if finite_mode {
            json!({"event":"starting", "operation":"finite_demand"})
        } else {
            json!({"event":"starting"})
        };
        let latest = Arc::new(Mutex::new((Instant::now(), initial)));
        let state = Arc::clone(&latest);
        // Rare state transitions must not disappear between sampled heartbeats.
        // Bound their storage too; a slow journal backpressures the publisher.
        let (stop, receiver) = mpsc::sync_channel(4);
        let handle = std::thread::spawn(move || {
            let mut terminal = if tty {
                let result = if finite_mode {
                    TerminalSession::try_new_finite(io::stderr())
                } else {
                    TerminalSession::try_new_family(io::stderr())
                };
                match result {
                    Ok(terminal) => Some(terminal),
                    Err(error) => {
                        if finite_mode && progress_enabled {
                            let _ = writeln!(
                                io::stderr().lock(),
                                "RustRed finite progress: terminal unavailable ({error}); using periodic status."
                            );
                        }
                        None
                    }
                }
            } else {
                None
            };
            let color = std::env::var_os("NO_COLOR").is_none();
            let started = Instant::now();
            let mut previous = None;
            let mut previous_cpu = process_cpu_seconds().map(|cpu| (started, cpu));
            let mut last_plain = None::<Instant>;
            loop {
                let finished = match receiver.recv_timeout(Duration::from_secs(1)) {
                    Ok(Control::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => true,
                    Ok(Control::Checkpoint(event)) => {
                        if let Err(error) = write_record(&mut *events, &event) {
                            cancellation.store(true, Ordering::Relaxed);
                            return Err(error);
                        }
                        false
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => false,
                };
                if stop_file.as_ref().is_some_and(|path| path.exists()) {
                    cancellation.store(true, Ordering::Relaxed);
                }
                let (observed, event) = state.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let counters = finite::local_snapshot(&event)
                    .unwrap_or_else(|| event.get("snapshot").unwrap_or(&event));
                let expanded = counters["completed_nodes"].as_u64().unwrap_or(0);
                let discovered = counters["scheduled_nodes"].as_u64().unwrap_or(0);
                let queued = counters["queued_nodes"].as_u64().unwrap_or(0);
                let now = Instant::now();
                let cpu = process_cpu_seconds();
                let cpu_cores = cpu.zip(previous_cpu).and_then(|(cpu, (time, prior))| {
                    let elapsed = now.duration_since(time).as_secs_f64();
                    (elapsed > 0. && cpu >= prior).then_some((cpu - prior) / elapsed)
                });
                previous_cpu = cpu.map(|cpu| (now, cpu));
                let sample = NodeSample {
                    time: now,
                    expanded,
                    queued,
                    window: finite::window_identity(&event),
                };
                let rates = sample.rates_since(previous, finite_mode);
                previous = Some(sample);
                let mut record = json!({"event":"heartbeat", "elapsed_seconds":started.elapsed().as_secs_f64(),
                    "process_rss_bytes":resident_set_bytes(), "cancel_requested":cancellation.load(Ordering::Relaxed),
                    "process_cpu_cores":cpu_cores,"reserved_workers":reserved_workers,
                    "progress_age_seconds":observed.elapsed().as_secs_f64(),
                    "expanded_nodes":expanded,"currently_discovered_nodes":discovered,
                    "recent_nodes_per_second":rates.map(|r|r.0),"queue_growth_per_second":rates.map(|r|r.1),
                    "progress_denominator_may_grow":true,"progress_is_not_closure_fraction":true,
                    "progress":event, "family_closure_claim":false});
                add_worker_summary(&mut record);
                finite::annotate_heartbeat(&mut record);
                if let Err(error) = write_record(&mut *events, &record) {
                    cancellation.store(true, Ordering::Relaxed);
                    return Err(error);
                }
                if let Some(display) = terminal.as_mut() {
                    let rendered = if finite_mode {
                        display.render_finite(&record, color)
                    } else {
                        let lines = dashboard(&record);
                        let refs = std::array::from_fn(|i| lines[i].as_str());
                        display.render_family(refs, color, cancellation.load(Ordering::Relaxed))
                    };
                    if let Err(error) = rendered {
                        terminal = None;
                        if finite_mode && progress_enabled {
                            let _ = writeln!(
                                io::stderr().lock(),
                                "RustRed finite progress: terminal drawing failed ({error}); using periodic status."
                            );
                        }
                    }
                }
                if finite_mode
                    && progress_enabled
                    && terminal.is_none()
                    && (finished
                        || last_plain
                            .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(30)))
                {
                    let _ = writeln!(io::stderr().lock(), "{}", finite::plain_status(&record));
                    last_plain = Some(now);
                }
                if finished {
                    break;
                }
            }
            Ok(())
        });
        Self {
            latest,
            stop,
            handle: Some(handle),
        }
    }
    pub fn observe(&self, mut event: Value) {
        let checkpoint = matches!(
            event["event"].as_str(),
            Some(
                "checkpoint_started"
                    | "finite_checkpoint_saved"
                    | "finite_checkpoint_restored"
                    | "checkpoint_saved"
                    | "checkpoint_restored"
                    | "checkpoint_skipped_unchanged"
                    | "checkpoint_executable_changed"
                    | "diagnostic_pause"
                    | "frontier_stop"
                    | "rescue_quarantine"
                    | "rescue_amendment_applied"
            )
        );
        let milestone = checkpoint.then(|| event.clone());
        // A skipped save carries no walk counters, and a diagnostic pause or an
        // A10 frontier stop is a one-off receipt: journal them as milestones
        // but leave the dashboard's latest progress record in place.
        if !matches!(
            event["event"].as_str(),
            Some(
                "checkpoint_skipped_unchanged"
                    | "diagnostic_pause"
                    | "frontier_stop"
                    | "rescue_quarantine"
                    | "rescue_amendment_applied"
            )
        ) {
            let mut latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
            finite::retain(&latest.1, &mut event);
            *latest = (Instant::now(), event);
        }
        if let Some(event) = milestone {
            let _ = self.stop.send(Control::Checkpoint(event));
        }
    }
    pub fn finish(mut self) -> io::Result<()> {
        let _ = self.stop.send(Control::Stop);
        self.handle
            .take()
            .unwrap()
            .join()
            .map_err(|_| io::Error::other("progress thread panicked"))?
    }
}

/// Process CPU time includes every native worker thread. Its wall-time delta
/// measures occupied cores independently of how many tasks the solver marks busy.
#[cfg(unix)]
fn process_cpu_seconds() -> Option<f64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `time` is valid writable storage for one libc timespec.
    if unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut time) } != 0 {
        return None;
    }
    Some(time.tv_sec as f64 + time.tv_nsec as f64 / 1e9)
}

#[cfg(not(unix))]
fn process_cpu_seconds() -> Option<f64> {
    None
}
impl Drop for RoutedProgress {
    fn drop(&mut self) {
        let _ = self.stop.send(Control::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn dashboard(record: &Value) -> [String; 6] {
    if record["progress"]["operation"].as_str() == Some("finite_demand") {
        return finite::dashboard(record);
    }
    if record["progress"]["operation"].as_str() == Some("owner_guarded_apply") {
        return guarded_dashboard(record);
    }
    let outer = &record["progress"];
    if outer["operation"].as_str() == Some("owner_domain_walk") {
        return walk_dashboard(record);
    }
    if outer["operation"].as_str() == Some("owner_domain_scan") {
        return domain_dashboard(record);
    }
    if outer["operation"].as_str() == Some("owner_domain_match") {
        return match_dashboard(record);
    }
    let p = if outer.get("snapshot").is_some() {
        &outer["snapshot"]
    } else {
        outer
    };
    let number = |name| p[name].as_u64().unwrap_or(0);
    let done = record["expanded_nodes"]
        .as_u64()
        .unwrap_or(number("completed_nodes"));
    let total = record["currently_discovered_nodes"]
        .as_u64()
        .unwrap_or(number("scheduled_nodes"));
    let filled = if total == 0 {
        0
    } else {
        (20.0 * (done as f64 / total as f64).min(1.0)) as usize
    };
    let bar = format!("[{}{}]", "#".repeat(filled), "-".repeat(20 - filled));
    let last_line = if let Some(detail) = p["first_failure"]["detail"].as_str() {
        let summary: String = detail.chars().take(200).collect();
        format!(
            "First failure ({} native calls draining): {summary}; full cause/origin in JSON",
            number("active_nodes")
        )
    } else {
        format!(
            "Last update {:.1}s ago; parametric family closure NOT established",
            record["progress_age_seconds"].as_f64().unwrap_or(0.)
        )
    };
    [
        format!(
            "RustRed shared finite-target campaign — {}",
            outer["status"]
                .as_str()
                .or(outer["phase"].as_str())
                .or(outer["event"].as_str())
                .unwrap_or("working")
        ),
        format!(
            "{bar} {done}/{total} expanded / currently discovered (may grow); {} queued / {} active / {} failed",
            number("queued_nodes"),
            number("active_nodes"),
            number("failed_nodes")
        ),
        format!(
            "Rules {}  routes {}  dedup {}  terminals {}  recent {:.1} nodes/s; queue {:+.1}/s",
            number("rule_applications"),
            number("transport_calls"),
            number("deduplication_hits"),
            number("declared_terminals"),
            record["recent_nodes_per_second"].as_f64().unwrap_or(0.),
            record["queue_growth_per_second"].as_f64().unwrap_or(0.)
        ),
        format!(
            "Missing owner/rule {}/{}; transport endpoint bound {}\nLocal node completion is not per-target closure. No ETA inferred.\nIBP generation/back-substitution: disabled",
            number("missing_owners"),
            number("missing_rules"),
            number("transport_endpoints")
        ),
        format!(
            "Elapsed {:.1}s  process RSS {:.3} GB  cancel {}",
            record["elapsed_seconds"].as_f64().unwrap_or(0.),
            record["process_rss_bytes"].as_u64().unwrap_or(0) as f64 / 1e9,
            record["cancel_requested"]
        ),
        last_line,
    ]
}

fn walk_dashboard(record: &Value) -> [String; 6] {
    let p = &record["progress"];
    let n = |key| p[key].as_u64().unwrap_or(0);
    let parallel = &p["parallel"];
    let pn = |key| parallel[key].as_u64().unwrap_or(0);
    let state = p["status"]
        .as_str()
        .or_else(|| {
            if p["event"].as_str() == Some("domain_draining") {
                Some("domain_draining")
            } else {
                p["phase"].as_str().or(p["event"].as_str())
            }
        })
        .unwrap_or("working");
    let last_line = if let Some(detail) = parallel["first_failure"]["detail"].as_str() {
        let summary: String = detail
            .chars()
            .filter(|c| !c.is_control())
            .take(200)
            .collect();
        let kind = if admission_worker_limits(parallel).is_some() {
            "native workers"
        } else {
            "workers"
        };
        format!(
            "First failure ({} {kind} draining): {summary}; full cause in JSON; NOT a closure claim",
            pn("active_workers")
        )
    } else if let Some(summary) = delegation_summary(p) {
        format!("{summary}; NOT a closure claim")
    } else {
        format!(
            "Last update {:.1}s ago; positive powers symbolic; NOT a closure claim",
            record["progress_age_seconds"].as_f64().unwrap_or(0.)
        )
    };
    let done = n("completed_nodes");
    let total = n("scheduled_nodes");
    let overlap = initial_overlap_summary(p);
    let inspected_label = if overlap.is_some() {
        "native jobs / scheduled"
    } else {
        "inspected / scheduled"
    };
    let (root_line, closure_line) = descendant_closure_summary(&p["descendant_closure"]);
    [
        format!("RustRed shared symbolic-domain work — {state}"),
        root_line,
        format!(
            "Successors {}  conditional {}  domain reuse {}  frontiers {}",
            n("successors"),
            n("conditional_successors"),
            n("deduplication_hits"),
            n("frontiers")
        ),
        format!(
            "Events {} committed / {} attempted  RSS {:.2} GB  elapsed {:.1}s\n{done}/{total} {inspected_label}; {} queued (may grow); entry {} / {} published\n{closure_line}{}",
            n("events"),
            pn("attempted_events"),
            record["process_rss_bytes"].as_u64().unwrap_or(0) as f64 / 1e9,
            record["elapsed_seconds"].as_f64().unwrap_or(0.),
            n("queued_nodes"),
            n("initial_entry_domains_published"),
            n("initial_entry_domains_total"),
            // Family dashboard slot 3 has room for multiple lines; slot 2
            // is single-line and would silently clip the overlap summary.
            overlap.map_or_else(String::new, |summary| format!("\n{summary}"))
        ),
        walk_worker_summary(parallel),
        last_line,
    ]
}

fn descendant_closure_summary(value: &Value) -> (String, String) {
    let counts = (|| {
        Some((
            value["initial_closed"].as_u64()?,
            value["initial_total"].as_u64()?,
            value["total_closed"].as_u64()?,
            value["total_domains"].as_u64()?,
            value["unresolved_domains"].as_u64()?,
        ))
    })();
    if let Some((roots, total, closed, domains, unresolved)) = counts
        && value["available"] == true
        && roots <= total
        && closed <= domains
        && unresolved == domains - closed
        && roots <= closed
    {
        let width = if total == 0 {
            0
        } else {
            (20.0 * roots as f64 / total as f64).min(20.0) as usize
        };
        let stale = if value["snapshot_stale"] == true {
            "; conservative stale snapshot"
        } else {
            ""
        };
        return (
            format!(
                "Roots [{}{}] {roots}/{total} dependency-closed{stale}",
                "#".repeat(width),
                "-".repeat(20 - width)
            ),
            format!(
                "Domains {closed}/{domains} recursively covered; {unresolved} unresolved; scoped worklist coverage, NOT termination/family certification"
            ),
        );
    }
    let reason = value["reason"]
        .as_str()
        .unwrap_or("dependency history unavailable")
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect::<String>();
    (
        "Roots [?-------------------] dependency closure unknown".into(),
        format!("Domain closure unavailable: {reason}; local publication is not closure"),
    )
}

/// Limits are reserved compute slots, not sampled activity. In particular no
/// helper-busy counter is inferred from its configured Rayon pool size.
fn admission_worker_limits(parallel: &Value) -> Option<(u64, u64, u64, u64)> {
    let admission = &parallel["admission_preparation"];
    Some((
        admission["inspection_worker_limit"].as_u64()?,
        admission["lookup_worker_limit"].as_u64()?,
        admission["coordinator_worker_limit"].as_u64()?,
        admission["requested_worker_budget"].as_u64()?,
    ))
}

fn walk_worker_summary(parallel: &Value) -> String {
    if let Some(observation) = parallel["activity_observation"].as_str() {
        let count = |key| {
            parallel[key]
                .as_u64()
                .map_or_else(|| "?".to_owned(), |value| value.to_string())
        };
        let state = match observation {
            "coordinator_sample" => "sampled",
            "inline_call_not_pollable" => "inline call not pollable",
            "joined" => "workers joined",
            "saved_cut_observation_not_live" => "saved cut, not live activity",
            _ => "activity unavailable",
        };
        return format!(
            "Epoch callbacks: {} computing / {} queued / {} returned awaiting publication ({state}; heartbeat age applies, not CPU utilization)",
            count("computing_workers"),
            count("queued_inspections"),
            count("finished_uncommitted_domains")
        );
    }
    let n = |key| parallel[key].as_u64().unwrap_or(0);
    if let Some((native, lookup, coordinator, budget)) = admission_worker_limits(parallel) {
        format!(
            "Native {}/{native} active ({} blocked); lookup {lookup} reserved + coordinator {coordinator}; total budget {budget}\n{} finished uncommitted; buffered {:.1} KiB logical",
            n("active_workers"),
            n("backpressured_workers"),
            n("finished_uncommitted_domains"),
            n("worker_buffered_logical_bytes") as f64 / 1024.
        )
    } else {
        format!(
            "Workers {} active / {} blocked / {} finished uncommitted; buffered {:.1} KiB logical",
            n("active_workers"),
            n("backpressured_workers"),
            n("finished_uncommitted_domains"),
            n("worker_buffered_logical_bytes") as f64 / 1024.
        )
    }
}

/// Non-TTY output remains one JSON object per line. Include the same bounded
/// human-readable labels as the TTY rather than inventing aggregate activity.
fn add_worker_summary(record: &mut Value) {
    if record["progress"]["operation"].as_str() == Some("owner_domain_walk") {
        record["worker_summary"] = json!(walk_worker_summary(&record["progress"]["parallel"]));
        if let Some(summary) = delegation_summary(&record["progress"]) {
            record["delegation_summary"] = json!(summary);
        }
        if let Some(summary) = initial_overlap_summary(&record["progress"]) {
            record["initial_overlap_summary"] = json!(summary);
        }
    }
}

/// A partial native job inspects only its residual. Reusing an initial band
/// retains the anchor obligation; it does not establish whole-domain closure.
fn initial_overlap_summary(progress: &Value) -> Option<String> {
    if progress["reuse_initial_d_bands"] != true {
        return None;
    }
    let partial = progress["partial_initial_inspections"]
        .as_u64()
        .unwrap_or(0);
    let blocked = progress["delegation"]["partial_initial_blocked"]
        .as_u64()
        .map_or_else(String::new, |n| format!(" ({n} locally blocked)"));
    Some(format!(
        "Initial-band partial inspections {partial}{blocked}; anchor obligations retained"
    ))
}

/// Transferred work is not inspected or solved; final ledger resolution is
/// separate from global frontiers. Never infer completion from cursor counts.
fn delegation_summary(progress: &Value) -> Option<String> {
    let ledger = progress.get("delegation")?;
    let transferred = ledger["transferred_obligations"].as_u64()?;
    let published = ledger["delegated_publications"].as_u64()?;
    let pending_native = ledger["pending_native_publications"].as_u64()?;
    let resolved = ledger["delegated_resolved"].as_u64().map_or_else(
        || "not yet resolved".into(),
        |n| format!("{n} locally discharged"),
    );
    Some(format!(
        "Delegated {transferred} ({published} published, {resolved}); native pending {pending_native}"
    ))
}

fn domain_dashboard(record: &Value) -> [String; 6] {
    let p = &record["progress"];
    let done = p["completed_owners"].as_u64().unwrap_or_else(|| {
        p["owners"].as_array().map_or(0, |owners| {
            owners
                .iter()
                .filter(|owner| owner["scan_complete"] == true)
                .count() as u64
        })
    });
    let total = p["total_owners"]
        .as_u64()
        .or(p["installed_owners"].as_u64())
        .unwrap_or(0);
    let filled = if total == 0 {
        0
    } else {
        ((20.0 * done as f64 / total as f64).min(20.0)) as usize
    };
    [
        format!("RustRed saved-rule domain scan — {}", p["status"].as_str()
            .or(p["phase"].as_str()).or(p["event"].as_str()).unwrap_or("working")),
        format!("[{}{}] {done}/{total} installed owners scanned", "#".repeat(filled), "-".repeat(20-filled)),
        format!("Owner {}  potential regions {}  retained summary groups {}",
            p["owner"].as_str().unwrap_or("—"), p["retained_regions"].as_u64().unwrap_or(0),
            p["summary_groups"].as_u64().unwrap_or(0)),
        "Positive powers remain parametric. Guard satisfiability and recursive closure are NOT established.\nNo concrete-target expansion or IBP regeneration.".into(),
        format!("Elapsed {:.1}s  process RSS {:.3} GB  cancel {}",
            record["elapsed_seconds"].as_f64().unwrap_or(0.),
            record["process_rss_bytes"].as_u64().unwrap_or(0) as f64 / 1e9, record["cancel_requested"]),
        format!("Last update {:.1}s ago; one-hop all-rule overapproximation, not a missing-rule verdict",
            record["progress_age_seconds"].as_f64().unwrap_or(0.)),
    ]
}

fn guarded_dashboard(record: &Value) -> [String; 6] {
    let p = &record["progress"];
    let n = |key| p[key].as_u64().unwrap_or(0);
    let id: String = p["id"]
        .as_str()
        .unwrap_or("—")
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect();
    [format!("RustRed conditional own-rule diagnostic — {}",p["status"].as_str().or(p["event"].as_str()).unwrap_or("working")),
        format!("{} / {} requested inspections finished; {} processed",n("completed_queries"),n("query_count"),n("processed_queries")),
        format!("Query {id}  retained events {}  payload charged {} bytes",n("retained_events"),n("report_payload_bytes_charged")),
        "Residual complements / RHS problems remain explicit; NOT first-priority applicability or closure.".into(),
        format!("Elapsed {:.1}s  process RSS {:.3} GB  cancel {}",record["elapsed_seconds"].as_f64().unwrap_or(0.),record["process_rss_bytes"].as_u64().unwrap_or(0) as f64/1e9,record["cancel_requested"]),
        "Completion means diagnostic collection only; no shared work queue or feasibility claim.".into()]
}

fn match_dashboard(record: &Value) -> [String; 6] {
    let p = &record["progress"];
    let done = p["completed_queries"].as_u64().unwrap_or(0);
    let processed = p["processed_queries"].as_u64().unwrap_or(done);
    let total = p["query_count"].as_u64().unwrap_or(0);
    let count = |name| p["counts"][name].as_u64().unwrap_or(0);
    // External IDs are labels, not terminal control sequences.
    let id: String = p["id"]
        .as_str()
        .unwrap_or("—")
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect();
    [
        format!(
            "RustRed local owner-domain classification — {}",
            p["status"]
                .as_str()
                .or(p["phase"].as_str())
                .or(p["event"].as_str())
                .unwrap_or("working")
        ),
        format!(
            "{processed}/{total} queries processed; {done} exactly classified; classification complete: {}",
            p["classification_complete"].as_bool().unwrap_or(false)
        ),
        format!(
            "Query {id}  pieces {}  selected rules {}  terminals {}  zero {}",
            p["retained_pieces"].as_u64().unwrap_or(0),
            count("selected_rule"),
            count("terminal"),
            count("exact_zero_sector")
        ),
        format!(
            "Exact gaps {}  unresolved {}  invalid conditions {}\nA classified gap is not applicability; RHS successors and recursive closure are NOT established.",
            count("exact_gap"),
            count("unresolved"),
            count("invalid_source_condition")
        ),
        format!(
            "Elapsed {:.1}s  process RSS {:.3} GB  cancel {}",
            record["elapsed_seconds"].as_f64().unwrap_or(0.),
            record["process_rss_bytes"].as_u64().unwrap_or(0) as f64 / 1e9,
            record["cancel_requested"]
        ),
        format!(
            "Last update {:.1}s ago; positive powers may remain unbounded. IBP generation disabled.",
            record["progress_age_seconds"].as_f64().unwrap_or(0.)
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_node_rates_reset_at_window_changes_and_missing_local_state() {
        let start = Instant::now();
        let previous = NodeSample {
            time: start,
            expanded: 1000,
            queued: 30,
            window: Some((3, 4)),
        };
        let next = NodeSample {
            time: start + Duration::from_secs(2),
            expanded: 1200,
            queued: 20,
            window: Some((3, 4)),
        };
        assert_eq!(next.rates_since(Some(previous), true), Some((100., -5.)));
        assert_eq!(
            NodeSample {
                window: Some((3, 5)),
                ..next
            }
            .rates_since(Some(previous), true),
            None
        );
        assert_eq!(
            NodeSample {
                window: Some((4, 0)),
                ..next
            }
            .rates_since(Some(previous), true),
            None
        );
        assert_eq!(
            NodeSample {
                window: None,
                ..next
            }
            .rates_since(Some(previous), true),
            None
        );
        assert_eq!(
            next.rates_since(
                Some(NodeSample {
                    window: None,
                    ..previous
                }),
                true
            ),
            None
        );
        assert_eq!(
            NodeSample {
                window: None,
                ..next
            }
            .rates_since(Some(previous), false),
            Some((100., -5.))
        );
    }

    #[test]
    fn rapid_checkpoint_milestones_survive_progress_replacement_and_finish() {
        struct Shared(Arc<Mutex<Vec<u8>>>);
        impl Write for Shared {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let monitor = RoutedProgress::start(
            Box::new(Shared(Arc::clone(&bytes))),
            false,
            Arc::new(AtomicBool::new(false)),
            None,
        );
        for generation in 1..=8 {
            monitor.observe(json!({"event":"checkpoint_started","generation":generation}));
            monitor.observe(json!({"event":"checkpoint_saved","generation":generation}));
            monitor.observe(json!({"event":"native_progress"}));
        }
        monitor.finish().unwrap();
        let text = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        let milestones: Vec<_> = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|row| row["event"] != "heartbeat")
            .collect();
        assert_eq!(milestones.len(), 16);
        for (index, row) in milestones.iter().enumerate() {
            assert_eq!(
                row["event"],
                if index % 2 == 0 {
                    "checkpoint_started"
                } else {
                    "checkpoint_saved"
                }
            );
            assert_eq!(row["generation"], index / 2 + 1);
        }
    }

    #[test]
    fn skipped_save_is_a_milestone_but_not_the_latest_progress_record() {
        struct Sink;
        impl Write for Sink {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let monitor = RoutedProgress::start(
            Box::new(Sink),
            false,
            Arc::new(AtomicBool::new(false)),
            None,
        );
        monitor.observe(json!({"event":"domain_progress","completed_nodes":7}));
        monitor.observe(json!({"event":"checkpoint_skipped_unchanged","generation":3}));
        let (_, latest) = monitor.latest.lock().unwrap().clone();
        assert_eq!(latest["event"], "domain_progress");
        assert_eq!(latest["completed_nodes"], 7);
        monitor.observe(json!({"event":"checkpoint_saved","generation":4}));
        let (_, latest) = monitor.latest.lock().unwrap().clone();
        assert_eq!(latest["event"], "checkpoint_saved");
        monitor.finish().unwrap();
    }

    #[test]
    fn diagnostic_pause_and_frontier_stop_are_journaled_but_not_the_latest_progress_record() {
        struct Shared(Arc<Mutex<Vec<u8>>>);
        impl Write for Shared {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let monitor = RoutedProgress::start(
            Box::new(Shared(Arc::clone(&bytes))),
            false,
            Arc::new(AtomicBool::new(false)),
            None,
        );
        monitor.observe(json!({"event":"domain_progress","completed_nodes":7}));
        monitor.observe(json!({"event":"diagnostic_pause",
            "diagnostic_pause":"ready-multi-prefix","ready_accepted_source_prefixes":2}));
        monitor.observe(
            json!({"event":"frontier_stop","stop_reason":"frontier_policy",
            "frontier_policy":"stop","frontiers":4,"session_start_frontiers":0}),
        );
        let (_, latest) = monitor.latest.lock().unwrap().clone();
        assert_eq!(latest["event"], "domain_progress");
        monitor.finish().unwrap();
        let text = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        let pauses: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|row| row["event"] == "diagnostic_pause")
            .collect();
        assert_eq!(pauses.len(), 1);
        assert_eq!(pauses[0]["ready_accepted_source_prefixes"], 2);
        let stops: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|row| row["event"] == "frontier_stop")
            .collect();
        assert_eq!(stops.len(), 1);
        assert_eq!(stops[0]["frontiers"], 4);
    }

    #[test]
    fn rescue_receipts_are_journaled_milestones() {
        struct Shared(Arc<Mutex<Vec<u8>>>);
        impl Write for Shared {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let monitor = RoutedProgress::start(
            Box::new(Shared(Arc::clone(&bytes))),
            false,
            Arc::new(AtomicBool::new(false)),
            None,
        );
        monitor.observe(json!({"event":"domain_progress","completed_nodes":3}));
        monitor.observe(json!({"event":"rescue_quarantine","quarantined_domains":5}));
        monitor.observe(json!({"event":"rescue_amendment_applied","sequence":1,"queries":2}));
        let (_, latest) = monitor.latest.lock().unwrap().clone();
        assert_eq!(latest["event"], "domain_progress");
        monitor.finish().unwrap();
        let text = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        let events: Vec<String> = text
            .lines()
            .map(|line| {
                serde_json::from_str::<Value>(line).unwrap()["event"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert!(
            events.contains(&"rescue_quarantine".to_owned()),
            "{events:?}"
        );
        assert!(
            events.contains(&"rescue_amendment_applied".to_owned()),
            "{events:?}"
        );
    }

    #[test]
    fn checkpoint_journal_failure_requests_cancellation_and_is_returned() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("test journal failure"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let monitor = RoutedProgress::start(Box::new(Broken), false, Arc::clone(&cancelled), None);
        monitor.observe(json!({"event":"checkpoint_saved"}));
        assert!(monitor.finish().is_err());
        assert!(cancelled.load(Ordering::Relaxed));
    }

    #[test]
    fn parallel_admission_dashboard_distinguishes_native_activity_from_reserved_helpers() {
        let mut record = json!({"progress":{"operation":"owner_domain_walk",
            "parallel":{"active_workers":2,"backpressured_workers":1,
                "finished_uncommitted_domains":3,"worker_buffered_logical_bytes":1024,
                "admission_preparation":{"inspection_worker_limit":25,"lookup_worker_limit":24,
                    "coordinator_worker_limit":1,"requested_worker_budget":50}}}});
        add_worker_summary(&mut record);
        let summary = record["worker_summary"].as_str().unwrap();
        assert!(summary.contains("Native 2/25 active (1 blocked)"));
        assert!(summary.contains("lookup 24 reserved + coordinator 1; total budget 50"));
        assert!(summary.contains("3 finished uncommitted; buffered 1.0 KiB logical"));
        assert!(!summary.contains("24 active"));
        assert!(!summary.contains("2/50"));
        assert_eq!(dashboard(&record)[4], summary);
        // A zero-work snapshot must not turn a configured helper into busy work.
        record["progress"]["parallel"]["active_workers"] = json!(0);
        record["progress"]["parallel"]["backpressured_workers"] = json!(0);
        assert!(
            dashboard(&record)[4].contains("Native 0/25 active (0 blocked); lookup 24 reserved")
        );
    }
    #[test]
    fn worker_summary_keeps_fallback_and_non_walk_heartbeats_unchanged() {
        let parallel = json!({"active_workers":2,"backpressured_workers":1,
            "finished_uncommitted_domains":3,"worker_buffered_logical_bytes":1024});
        let expected =
            "Workers 2 active / 1 blocked / 3 finished uncommitted; buffered 1.0 KiB logical";
        assert_eq!(walk_worker_summary(&parallel), expected);
        let mut incomplete = parallel.clone();
        incomplete["admission_preparation"] = json!({"lookup_worker_limit":24});
        assert_eq!(walk_worker_summary(&incomplete), expected);
        let mut other = json!({"progress":{"operation":"owner_domain_match","parallel":parallel}});
        let before = other.clone();
        add_worker_summary(&mut other);
        assert_eq!(other, before);
    }
    #[test]
    fn epoch_worker_summary_preserves_unknown_and_sampled_callback_semantics() {
        let mut record = json!({"progress":{"operation":"owner_domain_walk",
            "parallel":{"activity_observation":"inline_call_not_pollable",
                "active_workers":null,"computing_workers":null,
                "queued_inspections":null,"finished_uncommitted_domains":0}}});
        add_worker_summary(&mut record);
        let summary = record["worker_summary"].as_str().unwrap();
        assert!(summary.contains("? computing / ? queued / 0 returned"));
        assert!(summary.contains("inline call not pollable"));
        assert!(summary.contains("heartbeat age applies, not CPU utilization"));
        assert_eq!(dashboard(&record)[4], summary);
        record["progress"]["parallel"] = json!({"activity_observation":"coordinator_sample",
            "active_workers":2,"computing_workers":2,"queued_inspections":3,
            "finished_uncommitted_domains":4});
        add_worker_summary(&mut record);
        assert!(
            record["worker_summary"]
                .as_str()
                .unwrap()
                .contains("2 computing / 3 queued / 4 returned awaiting publication (sampled")
        );
        record["progress"]["parallel"] = json!({"activity_observation":"unavailable"});
        add_worker_summary(&mut record);
        assert!(
            record["worker_summary"]
                .as_str()
                .unwrap()
                .contains("? computing / ? queued / ? returned")
        );
    }
    #[test]
    fn guarded_dashboard_is_conditional_bounded_and_not_a_campaign_fraction() {
        let text=dashboard(&json!({"progress":{"operation":"owner_guarded_apply","event":"guarded_query_progress",
            "query_count":2,"completed_queries":1,"processed_queries":1,"retained_events":17,"id":format!("bad\u{1b}{}","x".repeat(1000))}})).join("\n");
        assert!(text.contains("1 / 2 requested inspections"));
        assert!(text.contains("retained events 17"));
        assert!(text.contains("diagnostic collection only"));
        assert!(!text.contains("finite-target"));
        assert!(!text.contains('\u{1b}'));
        assert!(text.len() < 1000);
    }
    #[test]
    fn owner_domain_walk_dashboard_reports_provisional_domain_work() {
        let text = dashboard(&json!({"progress":{"operation":"owner_domain_walk",
            "event":"domain_progress", "scheduled_nodes":7, "completed_nodes":2,
            "queued_nodes":5, "successors":100, "conditional_successors":3,
            "deduplication_hits":97, "frontiers":4}}))
        .join("\n");
        assert!(text.contains("2/7 inspected / scheduled"));
        assert!(text.contains("conditional 3"));
        assert!(text.contains("domain reuse 97"));
        assert!(text.contains("NOT a closure claim"));
        assert!(!text.contains("finite-target"));
        assert!(text.contains("dependency closure unknown"));
    }
    #[test]
    fn walk_bar_uses_transitive_initial_closure_not_local_publication() {
        let mut record = json!({"progress":{"operation":"owner_domain_walk",
            "scheduled_nodes":10,"completed_nodes":8,"initial_entry_domains_published":4,
            "initial_entry_domains_total":4,"descendant_closure":{"available":true,
            "initial_closed":1,"initial_total":4,"total_closed":3,"total_domains":10,
            "unresolved_domains":7,"snapshot_stale":true}}});
        let text = dashboard(&record).join("\n");
        assert!(text.contains("Roots [#####---------------] 1/4 dependency-closed"));
        assert!(text.contains("conservative stale snapshot"));
        assert!(text.contains("Domains 3/10 recursively covered; 7 unresolved"));
        assert!(text.contains("8/10 inspected / scheduled"));
        record["progress"]["descendant_closure"]["available"] = json!(false);
        assert!(dashboard(&record)[1].contains("unknown"));
        record["progress"]["descendant_closure"]["available"] = json!(true);
        record["progress"]["descendant_closure"]["initial_closed"] = json!(5);
        assert!(dashboard(&record)[1].contains("unknown"));
    }
    #[test]
    fn delegation_monitor_distinguishes_published_aliases_from_resolution() {
        let mut record = json!({"progress":{"operation":"owner_domain_walk",
            "completed_nodes":2,"scheduled_nodes":10,"queued_nodes":3,
            "delegation":{"transferred_obligations":5,"delegated_publications":5,
                "pending_native_publications":3}}});
        add_worker_summary(&mut record);
        let text = dashboard(&record).join("\n");
        assert!(text.contains("2/10 inspected / scheduled"));
        assert!(text.contains("5 published, not yet resolved"));
        assert!(text.contains("native pending 3"));
        assert!(text.contains("NOT a closure claim"));
        assert!(
            record["delegation_summary"]
                .as_str()
                .unwrap()
                .contains("not yet resolved")
        );
        record["progress"]["delegation"]["delegated_resolved"] = json!(4);
        assert!(
            dashboard(&record)
                .join("\n")
                .contains("4 locally discharged")
        );
    }
    #[test]
    fn initial_band_monitor_labels_partial_native_work_and_retained_anchors() {
        let mut record = json!({"progress":{"operation":"owner_domain_walk",
            "reuse_initial_d_bands":true,"partial_initial_inspections":3,
            "completed_nodes":7,"scheduled_nodes":12,"queued_nodes":5}});
        add_worker_summary(&mut record);
        let expected = "Initial-band partial inspections 3; anchor obligations retained";
        assert_eq!(record["initial_overlap_summary"], expected);
        let lines = dashboard(&record);
        assert!(lines[3].contains(expected));
        assert!(!lines[2].contains(expected));
        let text = lines.join("\n");
        assert!(text.contains("7/12 native jobs / scheduled"));
        assert!(!text.contains("7/12 inspected / scheduled"));
        assert!(text.contains(expected));
        assert!(text.contains("NOT a closure claim"));
        record["progress"]["delegation"] = json!({"partial_initial_blocked":2});
        add_worker_summary(&mut record);
        assert_eq!(
            record["initial_overlap_summary"],
            "Initial-band partial inspections 3 (2 locally blocked); anchor obligations retained"
        );
    }
    #[test]
    fn initial_band_monitor_keeps_default_and_non_walk_output_unchanged() {
        for enabled in [Value::Null, json!(false)] {
            let mut record = json!({"progress":{"operation":"owner_domain_walk",
                "reuse_initial_d_bands":enabled,"completed_nodes":2,"scheduled_nodes":3}});
            add_worker_summary(&mut record);
            assert!(record.get("initial_overlap_summary").is_none());
            assert!(
                dashboard(&record)
                    .join("\n")
                    .contains("2/3 inspected / scheduled")
            );
        }
        let mut other = json!({"progress":{"operation":"owner_domain_match",
            "reuse_initial_d_bands":true,"partial_initial_inspections":3}});
        let before = other.clone();
        add_worker_summary(&mut other);
        assert_eq!(other, before);
    }
    #[test]
    fn symbolic_dashboard_exposes_bounded_failure_and_backpressure_during_drain() {
        let text = dashboard(&json!({"progress":{"operation":"owner_domain_walk",
            "event":"domain_draining", "phase":"Apply", "events":17,
            "parallel":{"attempted_events":80, "active_workers":2,
                "backpressured_workers":1, "finished_uncommitted_domains":3,
                "worker_buffered_logical_bytes":1024,
                "first_failure":{"detail":format!("ResourceLimit\n\u{1b}[bad{}", "x".repeat(10000))}}}}))
            .join("\n");
        assert!(text.contains("— domain_draining"));
        assert!(text.contains("17 committed / 80 attempted"));
        assert!(text.contains("2 active / 1 blocked / 3 finished uncommitted"));
        assert!(text.contains("1.0 KiB logical"));
        assert!(text.contains("First failure (2 workers draining): ResourceLimit"));
        assert!(!text.contains('\u{1b}'));
        assert!(text.len() < 1000);
        assert!(text.contains("NOT a closure claim"));
    }
    #[test]
    fn owner_domain_match_dashboard_keeps_completed_gaps_distinct_from_coverage() {
        let text = dashboard(&json!({"progress":{"operation":"owner_domain_match",
            "status":"classified_with_gaps_or_invalid_conditions", "classification_complete":true,
            "completed_queries":2,"processed_queries":2,"query_count":2,"retained_pieces":3,
            "counts":{"selected_rule":1,"exact_gap":2}}}))
        .join("\n");
        assert!(text.contains("2/2 queries processed; 2 exactly classified"));
        assert!(text.contains("classification complete: true"));
        assert!(text.contains("Exact gaps 2"));
        assert!(text.contains("closure are NOT established"));
        assert!(!text.contains("expanded / currently discovered"));
    }

    #[test]
    fn owner_domain_match_dashboard_is_bounded_and_strips_label_controls() {
        let text = dashboard(&json!({"progress":{"operation":"owner_domain_match",
            "id":format!("\u{1b}[bad\n{}", "x".repeat(10000)),
            "processed_queries":2,"completed_queries":1,"query_count":3,
            "counts":{"unresolved":1}}}))
        .join("\n");
        assert!(!text.contains('\u{1b}'));
        assert!(text.contains("unresolved 1"));
        assert!(text.contains("2/3 queries processed; 1 exactly classified"));
        assert!(text.len() < 1000);
    }
    #[test]
    fn owner_scan_dashboard_reports_domains_not_concrete_reductions() {
        let text = dashboard(&json!({"progress":{"operation":"owner_domain_scan",
            "event":"owner_scan_progress","completed_owners":2,"total_owners":7,
            "retained_regions":1234}}))
        .join("\n");
        assert!(text.contains("2/7 installed owners scanned"));
        assert!(text.contains("potential regions 1234"));
        assert!(text.contains("NOT established"));
        assert!(!text.contains("expanded / currently discovered"));
    }
    #[test]
    fn dashboard_does_not_label_local_expansions_as_solved_targets() {
        let text = dashboard(&json!({"progress":{"completed_nodes":42}})).join("\n");
        assert!(text.contains("42/0 expanded"));
        assert!(text.contains("not per-target closure"));
        assert!(text.contains("NOT established"));
    }

    #[test]
    fn dashboard_displays_first_cause_before_other_native_calls_finish() {
        let text = dashboard(&json!({"progress":{"active_nodes":2,"failed_nodes":1,
            "first_failure":{"kind":"trace","detail":"ResourceLimit: transport endpoints"}}}))
        .join("\n");
        assert!(text.contains("First failure (2 native calls draining)"));
        assert!(text.contains("ResourceLimit: transport endpoints"));
        assert!(text.contains("full cause/origin in JSON"));
        assert!(text.contains("not per-target closure"));
    }
}
