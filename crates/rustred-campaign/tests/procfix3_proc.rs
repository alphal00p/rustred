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
