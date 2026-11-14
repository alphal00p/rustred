//! Plain mode (no terminal, `--plain`, `TERM=dumb`, after SIGHUP): single-line summaries and event lines, no escape
//! codes, suitable for a log file.

use std::io::Write;

use super::app::App;
use super::format::{count, cpu_hours, gib, percent, rough};
use super::model::{Banner, EventRow, ViewModel};

fn state_word(vm: &ViewModel, app: &App) -> String {
    if app.stop_count >= 2 {
        "STOPPING-FORCED".into()
    } else if app.stop_count == 1 {
        "STOPPING".into()
    } else {
        vm.banner.word().replace(' ', "-")
    }
}

/// One status line, UTC-prefixed (plan 3.U.2 plain mode).
pub fn status_line(vm: &ViewModel, app: &App) -> String {
    let utc = vm
        .now_utc
        .get(..19)
        .map(|s| format!("{s}Z"))
        .unwrap_or_else(|| vm.now_utc.clone());
    let r = &vm.resources;
    let eta = match (vm.eta.remaining_items, vm.eta.wall_s) {
        (Some(0), _) => "eta done".to_string(),
        (_, Some((lo, hi))) => format!("eta ~{}-{}", rough(lo), rough(hi)),
        _ => "eta ?".to_string(),
    };
    let defect = vm
        .defect_keys
        .map(|d| d.to_string())
        .unwrap_or_else(|| "-".into());
    let mut s = format!(
        "{utc} {} seq {} cpu {}/{} ram {}/{}G host {}G nodes {} (full {} boxed {} open {}) items-open {} rules {}/{} cert {} defect {} term {} cpu-h {} {eta}",
        state_word(vm, app),
        count(vm.seq),
        count(r.cpus_in_use),
        count(r.cpus_allowed),
        gib(r.rss_bytes),
        gib(r.mem_cap_bytes),
        gib(r.host_available_bytes),
        count(vm.nodes_total),
        count(vm.nodes_full),
        count(vm.nodes_boxed),
        count(vm.nodes_open),
        count(vm.items_open),
        count(vm.rules.accepted),
        count(vm.rules.rejected),
        percent(vm.certified_share),
        defect,
        count(vm.terminals_declared),
        cpu_hours(vm.cost_cpu_s),
    );
    if vm.banner == Banner::Stale {
        s.push_str(&format!(
            " hb-age {}",
            super::format::dur(vm.heartbeat_age_s)
        ));
    }
    if !vm.attention.is_empty() {
        s.push_str(&format!(" attention {}", vm.attention.len()));
    }
    if let Some(p) = vm.problems.first() {
        s.push_str(&format!(" problem: {p}"));
    }
    s.chars().filter(|c| !c.is_control()).collect()
}

/// One event line.
pub fn event_line(e: &EventRow) -> String {
    let s = format!("{} event {} {}", e.utc, e.kind, e.text);
    s.chars().filter(|c| !c.is_control()).collect()
}

/// Emits plain lines: new events (by seq) and a status line on a state change or every interval.
#[derive(Debug, Default)]
pub struct PlainPrinter {
    last_seq_printed: Option<u64>,
    last_state: Option<String>,
    last_status_s: Option<f64>,
}

impl PlainPrinter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lines to print now (events oldest first, then the status line if due).
    pub fn lines(&mut self, vm: &ViewModel, app: &App, now_s: f64, interval_s: f64) -> Vec<String> {
        let mut out = Vec::new();
        let mut new: Vec<&EventRow> = vm
            .events
            .iter()
            .filter(|e| self.last_seq_printed.is_none_or(|s| e.seq > s))
            .collect();
        new.sort_by_key(|e| e.seq);
        // on the first call, do not replay the whole history: the last 5 events
        if self.last_seq_printed.is_none() && new.len() > 5 {
            new.drain(..new.len() - 5);
        }
        for e in &new {
            out.push(event_line(e));
        }
        if let Some(m) = vm.events.iter().map(|e| e.seq).max() {
            self.last_seq_printed = Some(self.last_seq_printed.map_or(m, |s| s.max(m)));
        }
        let state = state_word(vm, app);
        let due = self.last_state.as_deref() != Some(state.as_str())
            || self.last_status_s.is_none_or(|t| now_s - t >= interval_s);
        if due {
            out.push(status_line(vm, app));
            self.last_state = Some(state);
            self.last_status_s = Some(now_s);
        }
        out
    }

    /// Writes [`PlainPrinter::lines`] to `w`.
    pub fn print(
        &mut self,
        w: &mut dyn Write,
        vm: &ViewModel,
        app: &App,
        now_s: f64,
        interval_s: f64,
    ) -> std::io::Result<()> {
        for l in self.lines(vm, app, now_s, interval_s) {
            writeln!(w, "{l}")?;
        }
        w.flush()
    }
}
