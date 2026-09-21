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
                let record = json!({"event":"heartbeat", "elapsed_seconds":started.elapsed().as_secs_f64(),
                    "process_rss_bytes":resident_set_bytes(), "cancel_requested":cancellation.load(Ordering::Relaxed),
                    "progress_age_seconds":observed.elapsed().as_secs_f64(),
                    "expanded_nodes":expanded,"currently_discovered_nodes":discovered,
                    "recent_nodes_per_second":rates.map(|r|r.0),"queue_growth_per_second":rates.map(|r|r.1),
                    "progress_denominator_may_grow":true,"progress_is_not_closure_fraction":true,
                    "progress":event, "family_closure_claim":false});
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
    let outer = &record["progress"];
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

#[cfg(test)]
mod tests {
    use super::*;
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
