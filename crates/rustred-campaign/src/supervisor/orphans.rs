//! The ORPHAN SCAN of the supervisor (plan 2.7.1; interfaces/supervisor.md section 8): a thin wrapper around the
//! process layer's verified primitives. The supervisor has NO signal path of its own (gate re-audit M-1): processes are
//! found with `resources::scan_campaign` (our uid, started late enough, carrying a marker of THIS campaign id) and
//! killed one by one with `launch::kill_orphan` (pidfd + exact start time + marker re-checked + boot id). This module
//! only NARROWS that set, never widens it: a ledger from another boot signals nothing, and the process's cwd must lie
//! under the campaign or work directory (coord-site-policy rule 5).

use crate::launch::kill_orphan;
use crate::resources::{MarkedProcess, scan_campaign};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// What the scan may touch.
#[derive(Clone, Debug)]
pub struct ScanRules {
    /// The campaign id of the marker (`RealConfig::campaign_id`; derived per campaign directory, section 8).
    pub campaign_id: String,
    /// Boot id recorded in the ledger for the attempts in question; another boot = signal nothing.
    pub recorded_boot_id: Option<String>,
    /// Processes started before this (clock ticks after boot) are never ours.
    pub min_start_ticks: u64,
    /// cwd must lie under one of these (canonicalised).
    pub roots: Vec<PathBuf>,
}

fn current_boot_id() -> Option<String> {
    crate::resources::boot_id().ok()
}

fn cwd_under(pid: u32, roots: &[PathBuf]) -> bool {
    match fs::read_link(format!("/proc/{pid}/cwd")) {
        Ok(cwd) => roots.iter().any(|r| cwd.starts_with(r)),
        Err(_) => false,
    }
}

impl ScanRules {
    /// True when the recorded boot is not this boot (nothing may be signalled).
    pub fn other_boot(&self) -> bool {
        match (&self.recorded_boot_id, current_boot_id()) {
            (Some(r), Some(now)) => *r != now,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }

    /// Marked processes of this campaign (optionally of one attempt id `<job>:a<k>`), narrowed by the cwd rule.
    pub fn scan(&self, only: Option<&str>) -> Vec<MarkedProcess> {
        if self.other_boot() {
            return Vec::new();
        }
        let Ok(s) = scan_campaign(&self.campaign_id, self.min_start_ticks, &BTreeSet::new()) else {
            return Vec::new();
        };
        s.processes
            .into_iter()
            .filter(|p| only.is_none_or(|o| o == p.attempt_id))
            .filter(|p| cwd_under(p.pid, &self.roots))
            .collect()
    }
}

/// SIGKILL every marked process (optionally of one attempt) through the verified path until none is left or `timeout`
/// passes. Returns the processes the signal was delivered to (each once). `progress(round, found)` is called per round.
pub fn sweep(
    rules: &ScanRules,
    only: Option<&str>,
    timeout: Duration,
    mut progress: impl FnMut(usize, usize),
) -> Vec<MarkedProcess> {
    let t0 = Instant::now();
    let mut killed: Vec<MarkedProcess> = Vec::new();
    let Some(boot) = current_boot_id() else {
        return killed;
    };
    let mut round = 0;
    loop {
        let found = rules.scan(only);
        if found.is_empty() {
            return killed;
        }
        round += 1;
        progress(round, found.len());
        for m in &found {
            if kill_orphan(&rules.campaign_id, &boot, m, libc::SIGKILL).delivered()
                && !killed
                    .iter()
                    .any(|k| k.pid == m.pid && k.start_ticks == m.start_ticks)
            {
                killed.push(m.clone());
            }
        }
        if t0.elapsed() > timeout {
            return killed;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Canonical form of a root for the cwd rule (falls back to the path itself).
pub fn canonical_root(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Start ticks of this process (the supervisor), the lower bound of everything it launches.
pub fn own_start_ticks() -> u64 {
    crate::resources::read_stat(std::process::id())
        .ok()
        .flatten()
        .map_or(0, |s| s.start_ticks)
}
