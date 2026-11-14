//! The ORPHAN SCAN of the supervisor (plan 2.7.1; interfaces/supervisor.md section 8): a thin wrapper around the
//! process layer's verified primitives. The supervisor has NO signal path of its own (gate re-audit M-1): processes are
//! found with `resources::scan_campaign` (our uid, started late enough, carrying a marker of THIS campaign id) and
//! killed one by one with `launch::kill_orphan` (pidfd + exact start time + marker re-checked + boot id). This module
//! only NARROWS that set, never widens it: a ledger from another boot signals nothing, and the process's cwd must lie
//! under the campaign or work directory (coord-site-policy rule 5).
//!
//! procfix3 (gate-B M1, gate-A M-1): the sweep is the process layer's FREEZING campaign sweep (`launch::sweep`: strict
//! scans with the exec-window re-read, SIGSTOP of every member through the guard until a fixed point, then SIGKILL, then
//! two conclusive empty scans). A marked process whose cwd lies outside the campaign directories is REPORTED (`left`,
//! ledger note `marked_survivor`), never signalled, and keeps its attempt from being settled.

use crate::launch::{Narrowing, SweepReport, SweepSpec};
use crate::resources::{MarkedProcess, scan_campaign};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

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
        self.scan_all(only)
            .into_iter()
            .filter(|p| cwd_under(p.pid, &self.roots))
            .collect()
    }

    /// Every live marked process of this campaign (optionally one attempt), WITHOUT the cwd narrowing: for reports.
    pub fn scan_all(&self, only: Option<&str>) -> Vec<MarkedProcess> {
        if self.other_boot() {
            return Vec::new();
        }
        let Ok(s) = scan_campaign(&self.campaign_id, self.min_start_ticks, &BTreeSet::new()) else {
            return Vec::new();
        };
        s.processes
            .into_iter()
            .filter(|p| only.is_none_or(|o| o == p.attempt_id))
            .collect()
    }

    /// The process layer's sweep spec with this module's narrowing (cwd rule). `None` for another boot: nothing may be
    /// signalled then.
    pub fn sweep_spec(&self, only: Option<&str>, budget: Duration) -> Option<SweepSpec> {
        if self.other_boot() {
            return None;
        }
        let roots = self.roots.clone();
        let narrowing: Narrowing = Arc::new(move |p: &MarkedProcess| {
            if cwd_under(p.pid, &roots) {
                Ok(())
            } else {
                Err(format!(
                    "cwd {} is outside the campaign directories (site rule 5): not signalled",
                    fs::read_link(format!("/proc/{}/cwd", p.pid))
                        .map(|c| c.display().to_string())
                        .unwrap_or_else(|e| format!("unreadable ({e})"))
                ))
            }
        });
        Some(SweepSpec {
            campaign_id: self.campaign_id.clone(),
            only_attempt: only.map(str::to_string),
            not_before_ticks: self.min_start_ticks,
            budget,
            pass_gap: Duration::from_millis(20),
            narrowing: Some(narrowing),
        })
    }
}

/// The freezing sweep of every marked process of this campaign (optionally one attempt), BLOCKING (restart time).
pub fn sweep(rules: &ScanRules, only: Option<&str>, budget: Duration) -> SweepReport {
    match rules.sweep_spec(only, budget) {
        Some(spec) => crate::launch::sweep::sweep(&spec, &AtomicBool::new(false)),
        None => SweepReport {
            proven: true,
            reason: None,
            ..Default::default()
        },
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
