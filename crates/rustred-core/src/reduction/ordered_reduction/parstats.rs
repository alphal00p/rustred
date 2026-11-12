//! rr-par diagnostics (switch, off by default): the serial fraction of a run.
//!
//! `RUSTRED_PAR_STATS=<file>` appends JSON lines describing where the wall
//! time of an Ordered request goes, so that the parallel and the serial part
//! of every stage can be told apart (an Amdahl table):
//!
//! - `{"stage":"forward",...}` once per request at the end of the forward
//!   pass: per phase the popped keys, the phase wall time, the wall time the
//!   main thread spent obtaining steps (waiting for the workers' windows) and
//!   the rest (the serial merge/record loop), and the window counters of the
//!   step source (windows, steps computed on the workers, their CPU seconds,
//!   steps used);
//! - `{"stage":"replay_job",...}` once per replay job (pass) with its lanes
//!   and wall seconds;
//! - `{"stage":"finish",...}` once per request with the finish split.
//!
//! Nothing here changes a decision of the run: it only reads clocks.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub(crate) fn path() -> Option<&'static std::path::Path> {
    static PATH: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| std::env::var_os("RUSTRED_PAR_STATS").map(Into::into))
        .as_deref()
}

pub(crate) fn enabled() -> bool {
    path().is_some()
}

pub(crate) fn write(line: &str) {
    use std::io::Write;
    let Some(path) = path() else {
        return;
    };
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// Counters of the step source (`Prefetched`), process-wide.
pub(crate) struct SourceCounters {
    pub windows: AtomicU64,
    pub window_nanos: AtomicU64,
    pub computed: AtomicU64,
    pub step_nanos: AtomicU64,
    pub used: AtomicU64,
    pub speculative: AtomicU64,
    pub speculative_hits: AtomicU64,
    pub wait_nanos: AtomicU64,
}

pub(crate) static SOURCE: SourceCounters = SourceCounters {
    windows: AtomicU64::new(0),
    window_nanos: AtomicU64::new(0),
    computed: AtomicU64::new(0),
    step_nanos: AtomicU64::new(0),
    used: AtomicU64::new(0),
    speculative: AtomicU64::new(0),
    speculative_hits: AtomicU64::new(0),
    wait_nanos: AtomicU64::new(0),
};

impl SourceCounters {
    pub(crate) fn take(&self) -> [u64; 8] {
        [
            self.windows.swap(0, Relaxed),
            self.window_nanos.swap(0, Relaxed),
            self.computed.swap(0, Relaxed),
            self.step_nanos.swap(0, Relaxed),
            self.used.swap(0, Relaxed),
            self.speculative.swap(0, Relaxed),
            self.speculative_hits.swap(0, Relaxed),
            self.wait_nanos.swap(0, Relaxed),
        ]
    }
}

pub(crate) fn nanos(elapsed: std::time::Duration) -> u64 {
    u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX)
}

/// Per-phase clock of the forward pass (main thread).
#[derive(Default)]
pub(crate) struct ForwardClock {
    on: bool,
    started: Option<std::time::Instant>,
    /// (phase, pops, phase wall, step-source wall, merge wall)
    phases: Vec<(usize, u64, f64, f64, f64)>,
}

impl ForwardClock {
    pub(crate) fn new() -> Self {
        let on = enabled();
        if on {
            let _ = SOURCE.take();
        }
        Self {
            on,
            started: on.then(std::time::Instant::now),
            phases: Vec::new(),
        }
    }
    #[inline]
    #[allow(dead_code)] // diagnostics API (rr-par), kept for callers that gate work on RUSTRED_PAR_STATS
    pub(crate) fn on(&self) -> bool {
        self.on
    }
    #[inline]
    pub(crate) fn now(&self) -> Option<std::time::Instant> {
        self.on.then(std::time::Instant::now)
    }
    pub(crate) fn phase(&mut self, phase: usize, pops: u64, wall: f64, source: f64, merge: f64) {
        if self.on {
            self.phases.push((phase, pops, wall, source, merge));
        }
    }
    pub(crate) fn report(&self, threads: usize) {
        if !self.on {
            return;
        }
        let total = self.started.map_or(0.0, |s| s.elapsed().as_secs_f64());
        let [
            windows,
            window_nanos,
            computed,
            step_nanos,
            used,
            speculative,
            hits,
            wait,
        ] = SOURCE.take();
        let phases: Vec<String> = self
            .phases
            .iter()
            .filter(|p| p.1 > 0)
            .map(|(phase, pops, wall, source, merge)| {
                format!("[{phase},{pops},{wall:.6},{source:.6},{merge:.6}]")
            })
            .collect();
        let (wall, source, merge): (f64, f64, f64) = self
            .phases
            .iter()
            .fold((0.0, 0.0, 0.0), |a, p| (a.0 + p.2, a.1 + p.3, a.2 + p.4));
        write(&format!(
            "{{\"stage\":\"forward\",\"threads\":{threads},\"total_seconds\":{total:.6},\
             \"phase_wall\":{wall:.6},\"source_wall\":{source:.6},\"merge_wall\":{merge:.6},\
             \"windows\":{windows},\"window_wall\":{:.6},\"computed\":{computed},\
             \"step_cpu\":{:.6},\"used\":{used},\"speculative\":{speculative},\
             \"speculative_hits\":{hits},\"wait_wall\":{:.6},\
             \"phases_pops_wall_source_merge\":[{}]}}",
            window_nanos as f64 * 1e-9,
            step_nanos as f64 * 1e-9,
            wait as f64 * 1e-9,
            phases.join(",")
        ));
    }
}
