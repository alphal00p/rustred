//! procfix3 regressions of the process layer (run ONLY as pid 1 of a private pid namespace: LIGHT_PIDNS=1, rule L10).
//!
//! gate-A M-1: a MARKED job process whose parent is a live job process that LOST the marker (exec'd with an empty or a
//! marker-free environment, own session, optionally `cd /`). Before procfix3 the end-of-job hunt never looked behind
//! the marker-less parent and reported `Verified` while the marked process lived. Now the walk descends through
//! marker-less processes (never signalling them) and the proof passes carry a full-scan marker belt: the marked process
//! is killed through the guard, the marker-less parent is reported (unattributed) and survives, then `Verified`.

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::launch::{
    CapPreference, ChildLauncher, RealConfig, RealLauncher, TrackingPreference, Verification,
};
use rustred_campaign::resources::{MARKER_VAR, read_stat};
use std::fs;
use std::path::Path;
use std::time::Duration;

fn require_namespace() {
    assert_eq!(std::process::id(), 1, "run as pid 1 of a private pid namespace");
}

fn launcher_ns() -> RealLauncher {
    RealLauncher::with_config(
        sampler(),
        RealConfig {
            cap_preference: CapPreference::NoSystemd,
            tracking: TrackingPreference::ProcOnly,
            verify_timeout: Duration::from_secs(30),
            rehunt_every: Duration::from_millis(300),
            ..RealConfig::default()
        },
    )
}

fn read_pid(p: &Path) -> u32 {
    wait_until("pid file", Duration::from_secs(30), || {
        fs::read_to_string(p).is_ok_and(|s| s.trim().parse::<u32>().is_ok())
    });
    fs::read_to_string(p).unwrap().trim().parse().unwrap()
}

fn marker_of(pid: u32) -> Option<String> {
    let b = fs::read(format!("/proc/{pid}/environ")).ok()?;
    b.split(|&c| c == 0)
        .filter_map(|e| std::str::from_utf8(e).ok())
        .find_map(|e| e.strip_prefix(&format!("{MARKER_VAR}=")).map(str::to_string))
}

fn live(pid: u32) -> bool {
    read_stat(pid)
        .ok()
        .flatten()
        .is_some_and(|s| !s.is_dead())
}

fn kill_own(pid: u32) {
    if live(pid) {
        // SAFETY: our own descendant inside our private pid namespace (we are pid 1 here; checked above).
        unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    }
}

fn run_variant(name: &str, exec_env: &str, cd: &str) {
    let _g = serial();
    require_namespace();
    let d = TempDir::new(name);
    let dir = d.path().to_path_buf();
    let mut l = launcher_ns();
    // A = setsid'd shell; it starts B (marked, inherits the marker) and then execs itself with a marker-less env.
    let script = format!(
        "setsid sh -c '{cd} sleep 300 & echo $! > {b}; echo $$ > {a}; exec {exec_env} sleep 300' & sleep 1; exit 0",
        b = dir.join("b.pid").display(),
        a = dir.join("a.pid").display(),
    );
    let spec = sh(&dir, &format!("pf3-{name}:a1"), &script);
    let mut h = l.launch(&spec).unwrap();
    let a = read_pid(&dir.join("a.pid"));
    let b = read_pid(&dir.join("b.pid"));
    std::thread::sleep(Duration::from_millis(300));
    assert!(marker_of(b).is_some(), "B carries the marker");
    let r = drive(&mut h);
    let (a_alive, b_alive) = (live(a), live(b));
    println!(
        "PF3 {name}: verification={:?} unattributed={:?} killed={:?} A {a} alive={a_alive} B {b} alive={b_alive}",
        r.verification,
        r.unattributed_children
            .iter()
            .map(|u| (u.pid, u.comm.clone()))
            .collect::<Vec<_>>(),
        r.leftovers_killed.iter().map(|k| k.pid).collect::<Vec<_>>()
    );
    kill_own(b);
    kill_own(a);
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        matches!(r.verification, Verification::Verified { .. }),
        "{name}: {:?}",
        r.verification
    );
    assert!(!b_alive, "{name}: the MARKED process {b} survived a Verified end");
    assert!(
        r.leftovers_killed.iter().any(|k| k.pid == b),
        "{name}: B {b} must be reported killed: {:?}",
        r.leftovers_killed
    );
    assert!(a_alive, "{name}: the marker-less process {a} is never signalled");
    assert!(
        r.unattributed_children.iter().any(|u| u.pid == a),
        "{name}: A {a} reported unattributed"
    );
}

#[test]
fn m1_marked_grandchild_behind_empty_environ_parent_is_killed_before_verified() {
    run_variant("g1empty", "env -i", "");
}

#[test]
fn m1_marked_grandchild_behind_marker_free_parent_cwd_root_is_killed_before_verified() {
    run_variant("g1unset", &format!("env -u {MARKER_VAR}"), "cd /;");
}

/// gate-A m-5: the tripwire extended beyond the signal syscalls (those stay in proc_reaudit_regressions::ra_m5): every
/// REAP form (waitid / wait4 / waitpid / `.wait()` / `try_wait` / `.status()` / `.output()`), SIGSTOP / SIGCONT,
/// cgroup.freeze / cgroup.kill, `raise(`, PR_SET_PDEATHSIG, ANY `Command::new(` (also with a variable path), the `nix` /
/// `rustix` crates, and SIGCHLD dispositions (SIG_IGN, SA_NOCLDWAIT) appear only in the files named here (with the
/// reason). Scanning stops only at a `#[cfg(test)]` that introduces a `mod` block (not at any `#[cfg(test)]` line).
#[test]
fn m5_extended_tripwire_reap_stop_spawn_forms_only_where_allowed() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let allowed: &[(&str, &[&str])] = &[
        // guard: waitid(P_PIDFD) of exact zombies (sys), SIGCHLD checks; real: peek_exit (WNOWAIT), the systemd probe
        // (own child, own session), try_wait under the registry lock; cli: the pre-flight version probe (separate
        // process, no launcher); the fake child (test double): wait4 of its own helpers.
        ("waitid", &["launch/guard.rs", "launch/real.rs"]),
        ("wait4", &["bin/rr_fake_child.rs"]),
        ("waitpid", &[]),
        (".wait()", &[]),
        ("try_wait", &["launch/real.rs", "cli/mod.rs"]),
        (".status()", &["launch/real.rs"]),
        (".output()", &[]),
        // stop / continue: only through the guard, from the hunt, the sweep and the launcher's terminate.
        ("SIGSTOP", &["launch/hunt.rs", "launch/sweep.rs"]),
        ("SIGCONT", &["launch/real.rs"]),
        ("cgroup.freeze", &["launch/cgroup.rs"]),
        ("cgroup.kill", &["launch/cgroup.rs"]),
        ("raise(", &[]),
        ("PR_SET_PDEATHSIG", &["launch/real.rs", "bin/rr_fake_child.rs"]),
        ("Command::new(", &["launch/real.rs", "cli/mod.rs"]),
        ("rustix::", &[]),
        ("SIG_IGN", &["launch/guard.rs", "bin/rr_fake_child.rs"]),
        ("SA_NOCLDWAIT", &["launch/guard.rs"]),
    ];
    let mut files = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    let mut hits = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&src).unwrap().display().to_string();
        let text = fs::read_to_string(f).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim_start().starts_with("#[cfg(test)]")
                && lines[i + 1..]
                    .iter()
                    .find(|l| !l.trim().is_empty())
                    .is_some_and(|l| l.trim_start().starts_with("mod "))
            {
                break;
            }
            let code = line.split("//").next().unwrap_or("");
            for (pat, ok) in allowed {
                if code.contains(pat) && !ok.contains(&rel.as_str()) {
                    hits.push(format!("{rel}:{}: [{pat}] {}", i + 1, line.trim()));
                }
            }
            // `nix::` as a path start (not `unix::`).
            if code.match_indices("nix::").any(|(k, _)| {
                k == 0 || !code[..k].chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_')
            }) {
                hits.push(format!("{rel}:{}: [nix::] {}", i + 1, line.trim()));
            }
        }
    }
    assert!(hits.is_empty(), "forms outside their allowed files: {hits:#?}");
    // Self-test: a planted snippet is caught.
    let planted = "fn f() { let _ = std::process::Command::new(p).status(); unsafe { libc::raise(9) }; }";
    assert!(allowed.iter().filter(|(p, _)| planted.contains(p)).count() >= 3);
}
