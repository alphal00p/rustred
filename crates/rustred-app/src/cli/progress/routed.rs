//! One bounded event slot and a heartbeat independent of native operation time.
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

pub(crate) struct RoutedProgress {
    latest: Arc<Mutex<(Instant, Value)>>,
    stop: mpsc::Sender<()>,
    handle: Option<JoinHandle<io::Result<()>>>,
}
impl RoutedProgress {
    pub fn start(
        mut events: Box<dyn Write + Send>,
        tty: bool,
        cancellation: Arc<AtomicBool>,
        stop_file: Option<PathBuf>,
    ) -> Self {
        let latest = Arc::new(Mutex::new((Instant::now(), json!({"event":"starting"}))));
        let state = Arc::clone(&latest);
        let (stop, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let mut terminal = if tty {
                TerminalSession::try_new_family(io::stderr()).ok()
            } else {
                None
            };
            let color = std::env::var_os("NO_COLOR").is_none();
            let started = Instant::now();
            let mut previous: Option<(Instant, u64, u64)> = None;
            loop {
                let finished = match receiver.recv_timeout(Duration::from_secs(1)) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => true,
                    Err(mpsc::RecvTimeoutError::Timeout) => false,
                };
                if stop_file.as_ref().is_some_and(|path| path.exists()) {
                    cancellation.store(true, Ordering::Relaxed);
                }
                let (observed, event) = state.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let counters = event.get("snapshot").unwrap_or(&event);
                let expanded = counters["completed_nodes"].as_u64().unwrap_or(0);
                let discovered = counters["scheduled_nodes"].as_u64().unwrap_or(0);
                let queued = counters["queued_nodes"].as_u64().unwrap_or(0);
                let now = Instant::now();
                let rates = previous.map(|(time, done, pending)| {
                    let seconds = now.duration_since(time).as_secs_f64().max(f64::EPSILON);
                    (
                        expanded.saturating_sub(done) as f64 / seconds,
                        (queued as f64 - pending as f64) / seconds,
                    )
                });
                previous = Some((now, expanded, queued));
                let mut record = json!({"event":"heartbeat", "elapsed_seconds":started.elapsed().as_secs_f64(),
                    "process_rss_bytes":resident_set_bytes(), "cancel_requested":cancellation.load(Ordering::Relaxed),
                    "progress_age_seconds":observed.elapsed().as_secs_f64(),
                    "expanded_nodes":expanded,"currently_discovered_nodes":discovered,
                    "recent_nodes_per_second":rates.map(|r|r.0),"queue_growth_per_second":rates.map(|r|r.1),
                    "progress_denominator_may_grow":true,"progress_is_not_closure_fraction":true,
                    "progress":event, "family_closure_claim":false});
                add_worker_summary(&mut record);
                if let Err(error) = serde_json::to_writer(&mut events, &record)
                    .map_err(io::Error::other)
                    .and_then(|()| events.write_all(b"\n"))
                    .and_then(|()| events.flush())
                {
                    cancellation.store(true, Ordering::Relaxed);
                    return Err(error);
                }
                if let Some(display) = terminal.as_mut() {
                    let lines = dashboard(&record);
                    let refs = std::array::from_fn(|i| lines[i].as_str());
                    if display
                        .render_family(refs, color, cancellation.load(Ordering::Relaxed))
                        .is_err()
                    {
                        terminal = None;
                    }
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
    pub fn observe(&self, event: Value) {
        *self.latest.lock().unwrap_or_else(|e| e.into_inner()) = (Instant::now(), event);
    }
    pub fn finish(mut self) -> io::Result<()> {
        let _ = self.stop.send(());
        self.handle
            .take()
            .unwrap()
            .join()
            .map_err(|_| io::Error::other("progress thread panicked"))?
    }
}
impl Drop for RoutedProgress {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn dashboard(record: &Value) -> [String; 6] {
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
    let width = if total == 0 {
        0
    } else {
        (20. * done as f64 / total as f64).min(20.) as usize
    };
    [
        format!("RustRed shared symbolic-domain work — {state}"),
        format!(
            "[{}{}] {done}/{total} inspected / scheduled; {} queued (may grow)",
            "#".repeat(width),
            "-".repeat(20 - width),
            n("queued_nodes")
        ),
        format!(
            "Successors {}  conditional {}  domain reuse {}  frontiers {}",
            n("successors"),
            n("conditional_successors"),
            n("deduplication_hits"),
            n("frontiers")
        ),
        format!(
            "Events {} committed / {} attempted  RSS {:.2} GB  elapsed {:.1}s",
            n("events"),
            pn("attempted_events"),
            record["process_rss_bytes"].as_u64().unwrap_or(0) as f64 / 1e9,
            record["elapsed_seconds"].as_f64().unwrap_or(0.)
        ),
        walk_worker_summary(parallel),
        last_line,
    ]
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
    }
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
