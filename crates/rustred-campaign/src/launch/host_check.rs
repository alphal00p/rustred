//! procfix7 round 11 (gate14 item 5: the auditor's design, notes/implementer.gate14.md "ITEM 5 SMALLEST SAFE HOST
//! CHECK"): THE SAFE HOST CHECK of the scope belt's `cgroup.kill` path on a REAL cgroupfs, for the COORDINATOR (prepared;
//! the implementer never runs it with `--run`). It is the only code of this crate meant to run on the host outside a
//! private pid namespace, and it touches nothing but ONE scope it creates itself under a unique name.
//!
//! Refusals BEFORE anything is started: the unit name is not `gate-scopecheck-<16 hex>`; inside a private pid namespace
//! (it needs the host's user manager); no cgroup v2; the user manager's cgroup root missing or not ours; the unit is
//! already loaded (`LoadState` must be `not-found`). Without `--run`: the plan is printed and nothing is started.
//!
//! With `--run`:
//! 1. C = `systemd-run --user --scope --unit=<U> -p MemoryMax=256M -p TasksMax=256 -- unshare -Urpf --mount-proc
//!    --kill-child -- /bin/sh -c '<churn>'`: every process of the scope except C itself lives in ITS OWN pid namespace
//!    (it can see and signal nothing outside);
//! 2. wait until C's `/proc/C/cgroup` names `<U>.scope`; the belt's record `ScopeRef::from_leader_cgroup` must accept
//!    it, and its directory must be `/sys/fs/cgroup` + systemd's `ControlGroup` of `<U>.scope`;
//! 3. READ-ONLY negatives: the record refuses the parent directory, the same path with ` (deleted)`, another unit name;
//!    a record with another inode number refuses to kill (nothing written: the scope stays populated, C alive);
//! 4. READ-ONLY: 10,000 x `live_members()` under the churn, the answers counted (Ok non-empty / Ok empty / Err populated
//!    without a live member / other Err): the round-10 F3 branch on a real cgroupfs;
//! 5. THE SAFETY GATE before the one write: every pid in the scope's subtree is C itself (comm `unshare`, the start time
//!    read at the spawn) or lives in the pid namespace of C's only child; anything else: ABORT without writing;
//! 6. THE WRITE: `kill_members()` (the belt's own call: descriptor-checked `cgroup.kill`), then within 5 s the scope
//!    unpopulated or removed, C killed by SIGKILL, the unit not loaded or inactive.
//!
//! Always at the end: if C still runs or the unit is still loaded, `systemctl --user stop <U>.scope` (the exact name), and
//! `reset-failed <U>.scope` if it failed (the exact name this check created). No pattern, no other pid, no other unit.
#![allow(clippy::disallowed_methods)] // a host tool outside the supervisor: its reads are reported, never collapsed

use super::cgroup::{ScopeRef, populated, procs_recursive, user_manager_root};
use crate::resources::{Bounded, procfs, read_stat, run_bounded};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const CHURN: &str = "while :; do (sleep 0.001 &); sleep 0.001; done";

/// Is `unit` `gate-scopecheck-` followed by exactly 16 lower-case hex digits?
pub fn valid_check_unit(unit: &str) -> bool {
    unit.strip_prefix("gate-scopecheck-").is_some_and(|h| {
        h.len() == 16
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// `systemctl --user show -p <prop> --value <unit>.scope`, bounded (10 s).
fn show(unit: &str, prop: &str) -> Result<String, String> {
    let (u, p) = (format!("{unit}.scope"), prop.to_string());
    match run_bounded(Duration::from_secs(10), move || {
        Command::new("systemctl")
            .args(["--user", "show", "-p", &p, "--value", &u])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
    }) {
        Bounded::Done(Ok(o)) if o.status.success() => {
            Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
        }
        Bounded::Done(Ok(o)) => Err(format!("systemctl show {prop}: {}", o.status)),
        Bounded::Done(Err(e)) => Err(format!("systemctl show {prop}: {e}")),
        Bounded::TimedOut => Err(format!("systemctl show {prop}: no answer within 10 s")),
        Bounded::SpawnFailed(e) => Err(format!("systemctl show {prop}: {e}")),
    }
}

/// `systemctl --user <verb> <unit>.scope` (the exact name), bounded (30 s).
fn systemctl(verb: &'static str, unit: &str) -> String {
    let u = format!("{unit}.scope");
    match run_bounded(Duration::from_secs(30), move || {
        Command::new("systemctl")
            .args(["--user", verb, &u])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    }) {
        Bounded::Done(Ok(s)) => format!("systemctl --user {verb} {unit}.scope: {s}"),
        Bounded::Done(Err(e)) => format!("systemctl --user {verb} {unit}.scope: {e}"),
        Bounded::TimedOut => format!("systemctl --user {verb} {unit}.scope: no answer within 30 s"),
        Bounded::SpawnFailed(e) => format!("systemctl --user {verb} {unit}.scope: {e}"),
    }
}

/// The refusals (nothing started). Ok(the user manager's cgroup root).
fn preconditions(unit: &str) -> Result<std::path::PathBuf, String> {
    if !valid_check_unit(unit) {
        return Err(format!(
            "refused: unit {unit:?} is not gate-scopecheck-<16 hex>"
        ));
    }
    if super::real::in_private_namespace() {
        return Err(
            "refused: inside a private pid namespace (the check needs the host's user manager)"
                .into(),
        );
    }
    match crate::fsx::presence("cgroup", Path::new("/sys/fs/cgroup/cgroup.controllers")) {
        crate::fsx::Presence::Present(_) => {}
        _ => return Err("refused: no cgroup v2 at /sys/fs/cgroup".into()),
    }
    let uid = crate::resources::our_uid();
    let root = user_manager_root(uid);
    match crate::fsx::presence("cgroup", &root) {
        crate::fsx::Presence::Present(md)
            if md.is_dir() && std::os::unix::fs::MetadataExt::uid(&md) == uid => {}
        _ => {
            return Err(format!(
                "refused: {} is not our user manager's cgroup",
                root.display()
            ));
        }
    }
    let load = show(unit, "LoadState")?;
    if load != "not-found" {
        return Err(format!(
            "refused: {unit}.scope is known to the user manager (LoadState {load})"
        ));
    }
    Ok(root)
}

/// Run the check. `say` receives every report line at once. Ok(()) = every step held; Err = a refusal (nothing started)
/// or a failed step (the scope was stopped by its exact name).
pub fn host_scope_check(unit: &str, run: bool, say: &mut dyn FnMut(&str)) -> Result<(), String> {
    let root = preconditions(unit)?;
    say(&format!(
        "preconditions hold: unit {unit}.scope not loaded, user manager cgroup {}",
        root.display()
    ));
    say(&format!(
        "plan: systemd-run --user --scope --unit={unit} -p MemoryMax=256M -p TasksMax=256 -- unshare -Urpf --mount-proc \
         --kill-child -- /bin/sh -c '{CHURN}'; record; read-only negatives; 10,000 live_members(); safety gate; ONE \
         cgroup.kill of that scope; stop {unit}.scope by its exact name if anything is left"
    ));
    if !run {
        say("DRY RUN: nothing started (add --run)");
        return Ok(());
    }
    let mut c = Command::new("systemd-run")
        .args([
            "--user",
            "--scope",
            "--quiet",
            &format!("--unit={unit}"),
            "-p",
            "MemoryMax=256M",
            "-p",
            "TasksMax=256",
        ])
        .args([
            "--",
            "unshare",
            "-Urpf",
            "--mount-proc",
            "--kill-child",
            "--",
            "/bin/sh",
            "-c",
            CHURN,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("step 1: systemd-run: {e}"))?;
    let cpid = c.id();
    let r = steps(unit, &mut c, cpid, say);
    // Always: nothing of the check is left (exact unit name only).
    let alive = matches!(c.try_wait(), Ok(None));
    if alive || !matches!(show(unit, "LoadState").as_deref(), Ok("not-found")) {
        say(&systemctl("stop", unit));
    }
    if matches!(show(unit, "ActiveState").as_deref(), Ok("failed")) {
        say(&systemctl("reset-failed", unit));
    }
    let t0 = Instant::now();
    while matches!(c.try_wait(), Ok(None)) && t0.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    say(&format!(
        "end: C {}, {unit}.scope LoadState {:?} ActiveState {:?}",
        match c.try_wait() {
            Ok(Some(s)) => format!("ended ({s})"),
            Ok(None) => "STILL RUNNING".into(),
            Err(e) => format!("unknown ({e})"),
        },
        show(unit, "LoadState"),
        show(unit, "ActiveState")
    ));
    r
}

fn steps(unit: &str, c: &mut Child, cpid: u32, say: &mut dyn FnMut(&str)) -> Result<(), String> {
    use std::os::unix::process::ExitStatusExt;
    let uid = crate::resources::our_uid();
    let cstart = read_stat(cpid)
        .map_err(|e| format!("step 1: stat of C {cpid}: {e}"))?
        .start_ticks;
    say(&format!("step 1: C = pid {cpid} (start {cstart})"));
    // 2. the record
    let want = format!("/{unit}.scope");
    let t0 = Instant::now();
    let text = loop {
        let t = procfs::sys()
            .cgroup_text(cpid, Duration::from_secs(5))
            .map_err(|e| format!("step 2: /proc/{cpid}/cgroup: {e}"))?;
        if t.lines()
            .any(|l| l.starts_with("0::") && l.trim_end().ends_with(&want))
        {
            break t;
        }
        if t0.elapsed() > Duration::from_secs(10) {
            return Err(format!("step 2: C never entered {unit}.scope: {t:?}"));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let r = ScopeRef::from_leader_cgroup(&text, cpid, uid, Some(unit))
        .ok_or_else(|| format!("step 2: the belt's record refused C's own scope: {text:?}"))?;
    let cg = show(unit, "ControlGroup")?;
    let sysd = Path::new("/sys/fs/cgroup").join(cg.trim_start_matches('/'));
    if r.dir != sysd {
        return Err(format!(
            "step 2: record {} != systemd's ControlGroup {}",
            r.dir.display(),
            sysd.display()
        ));
    }
    say(&format!(
        "step 2: record {} (dev {}, ino {}) == systemd's ControlGroup",
        r.dir.display(),
        r.dev,
        r.ino
    ));
    // 3. read-only negatives
    let rel = text
        .lines()
        .find_map(|l| l.strip_prefix("0::"))
        .unwrap_or_default()
        .trim_end()
        .to_string();
    let parent = rel
        .rsplit_once('/')
        .map(|(p, _)| p.to_string())
        .unwrap_or_default();
    for (what, t, u) in [
        (
            "the parent directory",
            format!("0::{parent}\n"),
            unit.to_string(),
        ),
        (
            "the path + ' (deleted)'",
            format!("0::{rel} (deleted)\n"),
            unit.to_string(),
        ),
        (
            "another unit name",
            text.clone(),
            "gate-scopecheck-0000000000000000".to_string(),
        ),
    ] {
        if ScopeRef::from_leader_cgroup(&t, cpid, uid, Some(&u)).is_some() {
            return Err(format!("step 3: the record accepted {what}"));
        }
    }
    let wrong = ScopeRef {
        ino: r.ino ^ 1,
        ..r.clone()
    };
    if wrong.kill_members().is_ok() {
        return Err("step 3: a record with another inode number killed".into());
    }
    if !matches!(populated(&r.dir), Ok(true)) || !matches!(c.try_wait(), Ok(None)) {
        return Err("step 3: the scope changed after the refused kill".into());
    }
    say(
        "step 3: refused the parent, ' (deleted)', another unit name, another inode (nothing written; C alive, scope populated)",
    );
    // 4. read-only F3 reachability
    let (mut nonempty, mut empty, mut f3, mut other) = (0u32, 0u32, 0u32, 0u32);
    let mut first_other = None;
    for _ in 0..10_000 {
        match r.live_members() {
            Ok(v) if v.is_empty() => empty += 1,
            Ok(_) => nonempty += 1,
            Err(e) if e.contains("populated") => f3 += 1,
            Err(e) => {
                other += 1;
                first_other.get_or_insert(e);
            }
        }
    }
    say(&format!(
        "step 4: 10,000 live_members(): Ok non-empty {nonempty}, Ok empty {empty}, Err populated-without-member {f3}, \
         other Err {other} {first_other:?}"
    ));
    // 5. the safety gate
    let kids =
        crate::resources::children_of(cpid).map_err(|e| format!("step 5: children of C: {e}"))?;
    let [child] = kids.iter().copied().collect::<Vec<_>>()[..] else {
        return Err(format!(
            "step 5: C has {} children (one expected): ABORT, nothing written",
            kids.len()
        ));
    };
    let ns = procfs::sys()
        .pid_ns_ino(child)
        .map_err(|e| format!("step 5: ns of C's child: {e}"))?;
    if procfs::sys().pid_ns_ino(cpid).ok() == Some(ns) {
        return Err("step 5: C's child shares C's pid namespace: ABORT, nothing written".into());
    }
    let members = procs_recursive(&r.dir).map_err(|e| format!("step 5: cgroup.procs: {e}"))?;
    let mut inside = 0usize;
    for p in &members {
        if *p == cpid {
            match read_stat(cpid) {
                Ok(st) if st.start_ticks == cstart && st.comm == "unshare" => continue,
                other => {
                    return Err(format!(
                        "step 5: C is not as spawned ({other:?}): ABORT, nothing written"
                    ));
                }
            }
        }
        match procfs::sys().pid_ns_ino(*p) {
            Ok(n) if n == ns => inside += 1,
            Err(crate::resources::ProcError::Gone) => {}
            other => {
                return Err(format!(
                    "step 5: pid {p} in the scope is not in C's namespace ({other:?}): ABORT, nothing written"
                ));
            }
        }
    }
    say(&format!(
        "step 5: safety gate holds: {} listed = C + {inside} in C's own pid namespace",
        members.len()
    ));
    // 6. THE WRITE
    r.kill_members()
        .map_err(|e| format!("step 6: cgroup.kill: {e}"))?;
    let t0 = Instant::now();
    let emptied = loop {
        match populated(&r.dir) {
            Ok(false) => break "unpopulated",
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break "removed",
            _ if t0.elapsed() > Duration::from_secs(5) => {
                return Err("step 6: the scope is still populated 5 s after cgroup.kill".into());
            }
            _ => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    let t0 = Instant::now();
    let status = loop {
        match c.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if t0.elapsed() < Duration::from_secs(5) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Ok(None) => return Err("step 6: C still runs 5 s after cgroup.kill".into()),
            Err(e) => return Err(format!("step 6: wait for C: {e}")),
        }
    };
    if status.signal() != Some(libc::SIGKILL) {
        return Err(format!("step 6: C ended with {status}, not SIGKILL"));
    }
    let t0 = Instant::now();
    let state = loop {
        let l = show(unit, "LoadState")?;
        let a = show(unit, "ActiveState")?;
        if l == "not-found" || a == "inactive" || t0.elapsed() > Duration::from_secs(5) {
            break (l, a);
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    say(&format!(
        "step 6: cgroup.kill written: scope {emptied}, C killed by SIGKILL, unit LoadState {} ActiveState {}",
        state.0, state.1
    ));
    if state.0 != "not-found" && state.1 != "inactive" {
        return Err(format!("step 6: the unit is still {state:?}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_unique_check_unit_is_accepted() {
        assert!(valid_check_unit("gate-scopecheck-0123456789abcdef"));
        for bad in [
            "gate-scopecheck-0123456789abcde",
            "gate-scopecheck-0123456789abcdef0",
            "gate-scopecheck-0123456789ABCDEF",
            "gate-scopecheck-0123456789abcdeg",
            "rr-camp-x",
            "gate-scopecheck-*",
            "",
        ] {
            assert!(!valid_check_unit(bad), "{bad}");
        }
    }

    /// Inside a private namespace (where the tests run) the check refuses before anything is started.
    #[test]
    fn the_check_refuses_inside_a_namespace_and_starts_nothing() {
        if !super::super::real::in_private_namespace() {
            return;
        }
        let mut lines = Vec::new();
        let r = host_scope_check("gate-scopecheck-0123456789abcdef", true, &mut |l| {
            lines.push(l.to_string())
        });
        assert!(
            matches!(&r, Err(e) if e.contains("private pid namespace")),
            "{r:?}"
        );
        assert!(lines.is_empty());
        let r = host_scope_check("bad", true, &mut |_| {});
        assert!(
            matches!(&r, Err(e) if e.contains("not gate-scopecheck")),
            "{r:?}"
        );
    }
}
