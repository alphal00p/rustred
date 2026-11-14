//! The campaign MARKER and the ORPHAN SCAN (plan 2.7.1; interfaces/supervisor.md section 8).
//!
//! Every child carries `RR_CAMPAIGN_JOB=<campaign key>:<job>:a<attempt>` in its environment, and every fork inherits
//! it. After a restart (and after every job end) the supervisor kills the processes that carry THIS campaign's
//! marker. Binding safety rules (directing session, after audit-proc-A C1): a process is signalled only if, for THAT
//! pid, right before the signal: (a) it is owned by our uid; (b) its start time is >= the recorded lower bound (and
//! equal to the recorded leader start when the pid is a recorded leader); (c) the boot id is the recorded one; (d) its
//! environ contains this campaign's marker (unreadable = not ours); (e) its cwd lies under one of the allowed roots.
//! The signal goes through `pidfd_open` + re-check + `pidfd_send_signal`: the pidfd pins the process, so a pid recycled
//! after the check is never hit. Nothing here uses `kill(-sid)` / `killpg` on a recorded number.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Environment variable of the marker.
pub const MARKER_ENV: &str = "RR_CAMPAIGN_JOB";

/// `<campaign key>:<job>:a<attempt>`.
pub fn marker_value(campaign_key: &str, job: &str, attempt: u32) -> String {
    format!("{campaign_key}:{job}:a{attempt}")
}

/// A process carrying this campaign's marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marked {
    pub pid: u32,
    pub start_ticks: u64,
    /// The part after the campaign key: `<job>:a<attempt>`.
    pub job_attempt: String,
}

/// What the scan may touch.
#[derive(Clone, Debug)]
pub struct ScanRules {
    pub campaign_key: String,
    /// Boot id recorded in the ledger for the attempts in question; if it differs from the current boot id, the scan
    /// signals nothing (every recorded process is gone).
    pub recorded_boot_id: Option<String>,
    /// Processes started before this (clock ticks after boot) are never ours.
    pub min_start_ticks: u64,
    /// Recorded leader identities (pid -> start ticks): such a pid is accepted only with exactly this start time.
    pub leaders: BTreeMap<u32, u64>,
    /// cwd must lie under one of these (canonicalised).
    pub roots: Vec<PathBuf>,
}

fn uid() -> u32 {
    // SAFETY: getuid has no preconditions and cannot fail.
    unsafe { libc::getuid() }
}

fn current_boot_id() -> Option<String> {
    fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|s| s.trim().to_string())
}

fn start_ticks_of(pid: u32) -> Option<u64> {
    let s = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = &s[s.rfind(')')? + 2..];
    // fields after ')' start at field 3 (state); start time is field 22 -> index 19.
    after.split_whitespace().nth(19)?.parse().ok()
}

fn state_of(pid: u32) -> Option<char> {
    let s = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    s[s.rfind(')')? + 2..].chars().next()
}

fn marker_in_environ(pid: u32, prefix: &str) -> Option<String> {
    let env = fs::read(format!("/proc/{pid}/environ")).ok()?;
    env.split(|&b| b == 0).find_map(|kv| {
        let kv = std::str::from_utf8(kv).ok()?;
        kv.strip_prefix(prefix).map(str::to_string)
    })
}

fn cwd_under(pid: u32, roots: &[PathBuf]) -> bool {
    match fs::read_link(format!("/proc/{pid}/cwd")) {
        Ok(cwd) => roots.iter().any(|r| cwd.starts_with(r)),
        Err(_) => false,
    }
}

impl ScanRules {
    fn prefix(&self) -> String {
        format!("{MARKER_ENV}={}:", self.campaign_key)
    }

    /// True when the recorded boot is not this boot (nothing may be signalled).
    pub fn other_boot(&self) -> bool {
        match (&self.recorded_boot_id, current_boot_id()) {
            (Some(r), Some(now)) => *r != now,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }

    /// The full check (a)-(e) for one pid; Some(marked) when it is ours.
    pub fn check(&self, pid: u32) -> Option<Marked> {
        if pid < 2 || pid == std::process::id() {
            return None;
        }
        let md = fs::metadata(format!("/proc/{pid}")).ok()?;
        if md.uid() != uid() {
            return None;
        }
        let st = start_ticks_of(pid)?;
        if st < self.min_start_ticks {
            return None;
        }
        if let Some(&want) = self.leaders.get(&pid)
            && want != st
        {
            return None;
        }
        let job_attempt = marker_in_environ(pid, &self.prefix())?;
        if !cwd_under(pid, &self.roots) {
            return None;
        }
        Some(Marked {
            pid,
            start_ticks: st,
            job_attempt,
        })
    }

    /// Every live (non-zombie) process of ours carrying this campaign's marker, optionally only `job_attempt`.
    pub fn scan(&self, only: Option<&str>) -> Vec<Marked> {
        if self.other_boot() {
            return Vec::new();
        }
        let Ok(rd) = fs::read_dir("/proc") else {
            return Vec::new();
        };
        let me = uid();
        let mut out = Vec::new();
        for e in rd.flatten() {
            let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            // Cheap filter first: owner.
            if e.metadata().map(|m| m.uid() != me).unwrap_or(true) {
                continue;
            }
            if state_of(pid) == Some('Z') {
                continue;
            }
            if let Some(m) = self.check(pid)
                && only.is_none_or(|o| o == m.job_attempt)
            {
                out.push(m);
            }
        }
        out.sort_by_key(|m| m.pid);
        out
    }
}

/// SIGKILL one verified process through a pidfd. Returns true when the signal was delivered.
pub fn kill_verified(rules: &ScanRules, m: &Marked) -> bool {
    if rules.other_boot() {
        return false;
    }
    // SAFETY: pidfd_open(pid, 0) takes plain integers; a negative return is an error.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, m.pid as libc::pid_t, 0) };
    if fd < 0 {
        return false;
    }
    let fd = fd as libc::c_int;
    // Re-check AFTER the pidfd pins the process: if the pid was recycled in between, the new process fails the check
    // (or, if it passes, it is ours too); if the pinned process died, the signal below reaches nothing.
    let ok = rules
        .check(m.pid)
        .is_some_and(|now| now.start_ticks == m.start_ticks);
    let mut sent = false;
    if ok {
        // SAFETY: fd is a valid pidfd we own; null siginfo; flags 0.
        let r = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd,
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        sent = r == 0;
    }
    // SAFETY: closing our own descriptor.
    unsafe { libc::close(fd) };
    sent
}

/// Kill every marked process (optionally of one attempt) until none is left or `timeout` passes. Returns the processes
/// killed (each at most once).
pub fn sweep(rules: &ScanRules, only: Option<&str>, timeout: Duration) -> Vec<Marked> {
    let t0 = Instant::now();
    let mut killed: Vec<Marked> = Vec::new();
    loop {
        let found = rules.scan(only);
        if found.is_empty() {
            return killed;
        }
        for m in &found {
            if kill_verified(rules, m)
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
    start_ticks_of(std::process::id()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    fn rules(key: &str, root: &Path) -> ScanRules {
        ScanRules {
            campaign_key: key.into(),
            recorded_boot_id: current_boot_id(),
            min_start_ticks: 0,
            leaders: BTreeMap::new(),
            roots: vec![canonical_root(root)],
        }
    }

    #[test]
    fn only_marked_processes_of_this_campaign_are_killed() {
        let dir = std::env::temp_dir().join(format!("rr-sup-orph-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let spawn = |marker: Option<&str>| {
            let mut c = Command::new("/bin/sleep");
            c.arg("60").current_dir(&dir).env_clear();
            if let Some(m) = marker {
                c.env(MARKER_ENV, m);
            }
            // SAFETY: setsid is async-signal-safe.
            unsafe {
                c.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }
            c.spawn().unwrap()
        };
        let mut ours = spawn(Some("camp@aaaa:j1:a1"));
        let mut decoy = spawn(None);
        let mut other = spawn(Some("camp@bbbb:j1:a1"));
        std::thread::sleep(Duration::from_millis(100));
        let mut r = rules("camp@aaaa", &dir);
        // A recorded leader identity with a wrong start time naming the decoy's pid: still untouched.
        r.leaders.insert(decoy.id(), 1);
        let found = r.scan(None);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].pid, ours.id());
        assert_eq!(found[0].job_attempt, "j1:a1");
        // Another boot: nothing.
        let mut ob = r.clone();
        ob.recorded_boot_id = Some("not-this-boot".into());
        assert!(ob.scan(None).is_empty());
        assert!(!kill_verified(&ob, &found[0]));
        // Wrong root: nothing.
        let mut wr = r.clone();
        wr.roots = vec![PathBuf::from("/nonexistent-root")];
        assert!(wr.scan(None).is_empty());
        let killed = sweep(&r, None, Duration::from_secs(5));
        assert_eq!(killed.len(), 1);
        let st = ours.wait().unwrap();
        assert!(st.code().is_none());
        assert!(decoy.try_wait().unwrap().is_none(), "decoy must survive");
        assert!(other.try_wait().unwrap().is_none(), "other campaign must survive");
        let _ = decoy.kill();
        let _ = other.kill();
        let _ = decoy.wait();
        let _ = other.wait();
        let _ = fs::remove_dir_all(&dir);
    }
}
