//! The ORPHAN SCAN of the supervisor (plan 2.7.1; interfaces/supervisor.md section 8): a thin wrapper around the
//! process layer's verified primitives. The supervisor has NO signal path of its own (gate re-audit M-1): processes are
//! found with `resources::scan_campaign` (our uid, started late enough, carrying a marker of THIS campaign id) and
//! killed one by one through the guard (pidfd + exact start time + marker re-checked + boot id). This module only
//! NARROWS that set, never widens it: a ledger from another boot signals nothing.
//!
//! procfix3 (gate-B M1, gate-A M-1): the sweep is the process layer's FREEZING campaign sweep (`launch::sweep`: strict
//! scans with the exec-window re-read, SIGSTOP of every member through the guard until a fixed point, then SIGKILL, then
//! two conclusive empty scans).
//!
//! Lane int5 (gate3 M1): NO cwd rule any more. Ownership is the MARKER with the identity checklist (THE ONE OWNERSHIP
//! RULE, `launch::guard::marker_target`: this campaign's id, which binds the directory's nonce, device and inode; the
//! attempt label; our uid; this boot; a start time not before the bound): a process carrying this campaign's exact
//! marker is ours wherever its cwd is, and is killed through the guarded path and recorded (`orphan_killed`,
//! `signal_sent`). The former rule (only report a marked orphan whose cwd lies outside the campaign directories) left a
//! double-forked, setsid'd daemon that did chdir("/") alive forever and its attempt unsettled. What a sweep cannot
//! verify or signal is reported (`left`) and stops the campaign with exit 8 (never exit 0) listing the pids.

use crate::launch::{SweepReport, SweepSpec};

use std::path::{Path, PathBuf};
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
    /// The campaign and work directories (canonicalised): only for the log annotation of a kill (cwd inside / outside);
    /// NOT an ownership criterion (gate3 M1).
    pub roots: Vec<PathBuf>,
}

/// The current boot id; None when it cannot be read (procfix7 B1: unknown is never "another boot").
fn current_boot_id() -> Option<String> {
    // procfix8-ok: unknown (Gone or Unreadable) is None, and `other_boot` answers false for it (never "another boot")
    crate::resources::boot_id().ok()
}

/// The cwd of `pid` relative to the campaign directories, for logs only ("inside" / "outside <cwd>" / "unreadable").
pub fn cwd_note(pid: u32, roots: &[PathBuf]) -> String {
    match crate::resources::procfs::sys().cwd(pid) {
        Ok(cwd) if roots.iter().any(|r| cwd.starts_with(r)) => {
            "cwd inside the campaign directories".into()
        }
        Ok(cwd) => format!("cwd {} outside the campaign directories", cwd.display()),
        Err(e) => format!("cwd unreadable ({e})"),
    }
}

impl ScanRules {
    /// True when the recorded boot is PROVED not this boot (nothing may be signalled). procfix7 (gate7 B1, the class):
    /// an unreadable current boot id is not "another boot" (that made a sweep "proven" with nothing scanned); it is
    /// left to the sweep, which then reports itself unproven.
    pub fn other_boot(&self) -> bool {
        match (&self.recorded_boot_id, current_boot_id()) {
            (Some(r), Some(now)) => *r != now,
            (Some(_), None) | (None, _) => false,
        }
    }

    /// The process layer's sweep spec: THE ONE OWNERSHIP RULE, no narrowing of the supervisor's own (gate3 M1). `None`
    /// for another boot: nothing may be signalled then.
    pub fn sweep_spec(&self, only: Option<&str>, budget: Duration) -> Option<SweepSpec> {
        if self.other_boot() {
            return None;
        }
        Some(SweepSpec {
            campaign_id: self.campaign_id.clone(),
            only_attempt: only.map(str::to_string),
            not_before_ticks: self.min_start_ticks,
            budget,
            pass_gap: Duration::from_millis(20),
            narrowing: None,
        })
    }
}

/// The freezing sweep of every marked process of this campaign (optionally one attempt), BLOCKING (restart time).
pub fn sweep(rules: &ScanRules, only: Option<&str>, budget: Duration) -> SweepReport {
    sweep_seeded(rules, only, &[], budget)
}

/// procfix7 round 6: [`sweep`] with SEEDED suspects (`launch::sweep::sweep_seeded`): pids the ledger recorded, each
/// proven dead or not ours by its own pid.
pub fn sweep_seeded(
    rules: &ScanRules,
    only: Option<&str>,
    seeds: &[(u32, Option<u64>)],
    budget: Duration,
) -> SweepReport {
    match rules.sweep_spec(only, budget) {
        Some(spec) => crate::launch::sweep::sweep_seeded(&spec, seeds, &AtomicBool::new(false)),
        None => SweepReport {
            proven: true,
            reason: None,
            ..Default::default()
        },
    }
}

/// Canonical form of a root for the cwd rule (falls back to the path itself).
pub fn canonical_root(p: &Path) -> PathBuf {
    crate::fsx::canonicalize("supervisor.dirs", p).unwrap_or_else(|_| p.to_path_buf())
}

/// Start ticks of this process (the supervisor), the lower bound of everything it launches. procfix8: an unreadable own
/// stat gives 0 (the WIDEST bound: every process since boot is a candidate), never a narrower one.
pub fn own_start_ticks() -> u64 {
    match crate::resources::read_stat(std::process::id()) {
        Ok(s) => s.start_ticks,
        Err(_) => 0,
    }
}
