//! gate11 audit (WS/notes/implementer.gate11.md), finding G11-M1: THE COMPLETENESS WALK IS BLIND TO CHILDREN OF NON-LEADER
//! THREADS. `ProcRoot::pids_proven` (resources/procfs.rs) proves a /proc listing complete against a walk of the kernel's
//! children lists that reads `/proc/<p>/task/<p>/children` only (the LEADER thread's list), from pid 1 and from the
//! supervisor. A process forked by a NON-LEADER thread is on `/proc/<p>/task/<tid>/children` and is never reached; neither
//! is its subtree. A listing that lacks exactly such a process (both enumerations, in agreement: gate10 G10-B1's injected
//! fault, generalised to a filter) is accepted as complete.
//!
//! - `g11_walk_unit_*` (ignored, FAILS on fdec15da): in-process, the test-build truncation `Faults::truncate_root(n - 1)`
//!   drops the newest process, a `sleep` started by a non-leader thread of a live python3: `pids_proven` answers Ok.
//! - `g11_walk_e2e_walk_blind_marked_survivor` (ignored, FAILS on fdec15da): a crashed session whose job started an UNMARKED
//!   setsid'd parent whose non-leader thread forked a MARKED sleep; session 2 under scripts/gate11/src/g11filter.c (LD_PRELOAD:
//!   both enumerations drop that one pid) ends EXIT 0 with the marked process ALIVE. Needs the namespace test override
//!   `resources.containment = "marker_only"` (on the host the job scope's cgroup.kill covers it: RUNBOOK 2.9).
//! - `g11_walk_e2e_reached_daemon_drop_is_caught` (control, passes): the same filter on a daemon the walk DOES reach
//!   (re-parented to the namespace's init) is caught (typed refusal / exit 8), never exit 0 with it alive.
//!
//! Run ONLY inside a private pid namespace: `LIGHT_PIDNS=1 light_build.sh gate11 test -p rustred-campaign --test
//! gate11_walk -- --include-ignored`. Needs a C compiler and python3 (skips otherwise).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::resources::procfs::{Faults, ProcRoot};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

fn in_ns() -> bool {
    if rustred_campaign::launch::in_private_namespace() {
        return true;
    }
    eprintln!(
        "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
    );
    false
}

fn path_env() -> std::ffi::OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| s.rfind(')').map(|i| !s[i + 2..].starts_with('Z')))
        .unwrap_or(false)
}

/// Test code: SIGKILL one exact pid of ours (the shell's builtin).
fn kill_pid(pid: u32) {
    let _ = Command::new("sh")
        .args(["-c", &format!("kill -KILL {pid} 2>/dev/null")])
        .env("PATH", path_env())
        .status();
}

fn src(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/gate11/src")
        .join(name)
}

/// Compile one helper (shared object when `so`), None without a C compiler.
fn cc(t: &Tmp, name: &str, so: bool) -> Option<PathBuf> {
    let out = t
        .path
        .join(name.trim_end_matches(".c").to_string() + if so { ".so" } else { "" });
    let mut c = Command::new("cc");
    if so {
        c.args(["-shared", "-fPIC"]);
    }
    c.args(["-O1", "-o"]).arg(&out).arg(src(name));
    c.args(if so { vec!["-ldl"] } else { vec!["-lpthread"] });
    let ok = c
        .env("PATH", path_env())
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("SKIPPED: no C compiler for {name}");
        return None;
    }
    Some(out)
}

/// The unit-level proof: a live process older than the listing, forked by a NON-LEADER thread, missing from a listing
/// truncated in agreement, must make `pids_proven` Unreadable.
#[test]
#[ignore = "gate11 G11-M1: FAILS on fdec15da (the walk reads leader children lists only)"]
fn g11_walk_unit_child_of_a_non_leader_thread_is_never_proven_absent() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    // python3: a NON-LEADER thread starts `sleep 600` (its parent is that thread) and prints the pid
    let script = "import threading, subprocess, sys, time\n\
                  def w():\n    p = subprocess.Popen(['sleep', '600'])\n    print(p.pid, flush=True)\n    time.sleep(600)\n\
                  threading.Thread(target=w, daemon=True).start()\ntime.sleep(600)\n";
    let Ok(mut py) = Command::new("python3")
        .args(["-I", "-c", script])
        .env("PATH", path_env())
        .stdout(Stdio::piped())
        .spawn()
    else {
        eprintln!("SKIPPED: no python3");
        return;
    };
    let mut line = String::new();
    std::io::BufReader::new(py.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let m: u32 = line.trim().parse().unwrap();
    std::thread::sleep(Duration::from_millis(300)); // older than the listing by far more than 2 ticks
    let pp = py.id();
    let leader_list = std::fs::read_to_string(format!("/proc/{pp}/task/{pp}/children")).unwrap();
    assert!(
        !leader_list.split_whitespace().any(|w| w == m.to_string()),
        "precondition: {m} is not on the leader's children list"
    );
    let full = ProcRoot::system().pids_proven(&[]).expect("full listing");
    assert_eq!(
        full.last(),
        Some(&m),
        "precondition: the thread's child is the newest process"
    );
    let cut = ProcRoot::system().with_faults(Faults::new().truncate_root(full.len() - 1));
    let r = cut.pids_proven(&[]);
    kill_pid(m);
    let _ = py.kill();
    let _ = py.wait();
    assert!(
        r.is_err(),
        "pids_proven accepted a listing that lacks the live process {m} (a child of a non-leader thread): {r:?}"
    );
}

/// Session 1: its first job starts HELPER (`thread`: g11thr, an unmarked setsid'd parent whose non-leader thread forks a
/// MARKED sleep; `daemon`: a marked setsid'd double-forked sleep); SIGKILLed once it runs. Returns (spec, cdir, marked pid,
/// helper parent pid or 0).
fn crashed_session(t: &Tmp, helper: &str, thr: &Path) -> (PathBuf, PathBuf, u32, u32) {
    let rec = t.path.join("helper.pids");
    let launch = match helper {
        "thread" => format!(
            "env -i {} {rec}.w \"$RR_CAMPAIGN_JOB\" thread & \
             for i in $(seq 1 50); do [ -s {rec}.w ] && break; sleep 0.1; done; mv {rec}.w {rec}",
            thr.display(),
            rec = rec.display()
        ),
        _ => format!(
            "( setsid sleep 600 </dev/null >/dev/null 2>&1 & echo \"$! 0\" > {rec}.tmp; mv {rec}.tmp {rec} )",
            rec = rec.display()
        ),
    };
    let wrap = t.path.join("wrap.sh");
    std::fs::write(
        &wrap,
        format!(
            "#!/bin/sh\nif [ \"$1\" = job ] && mkdir {rec}.lock 2>/dev/null; then\n  {launch}\nfi\nexec {fake} \"$@\"\n",
            rec = rec.display(),
            fake = FAKE
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrap, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut v = spec(
        t,
        serde_json::json!({"items": 4, "nodes": 1, "rungs": 1, "seed": 5, "case_ms": [1500, 2500],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 2, "unproven_end_limit_s": 60}),
    );
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    let sp = write_spec(t, "spec.json", &v);
    let d = t.cdir("c");
    let mut s1 = start(
        &sp,
        &d,
        &["--plain"],
        &[("PATH", path_env().to_str().unwrap())],
    );
    wait_until("the helper", Duration::from_secs(60), || rec.exists());
    std::thread::sleep(Duration::from_millis(500));
    signal(&s1, libc::SIGKILL);
    let _ = s1.wait();
    let f = std::fs::read_to_string(&rec).unwrap();
    let mut it = f.split_whitespace().map(|w| w.parse::<u32>().unwrap_or(0));
    let m = it.next().unwrap();
    let p = it.next().unwrap_or(0);
    assert!(alive(m), "precondition: the marked process lives");
    (sp, d, m, p)
}

/// Session 2 under the listing filter that drops exactly `m` from both enumerations. Returns (exit code, m alive).
fn filtered_session2(t: &Tmp, sp: &Path, d: &Path, m: u32, so: &Path) -> (i32, bool) {
    let drop = t.path.join("drop.txt");
    std::fs::write(&drop, format!("{m}\n")).unwrap();
    let log = t.path.join("filter.log");
    let path = path_env();
    let env = [
        ("LD_PRELOAD", so.to_str().unwrap()),
        ("G11_DROP_FILE", drop.to_str().unwrap()),
        ("G11_LOG", log.to_str().unwrap()),
        ("PATH", path.to_str().unwrap()),
    ];
    let mut c = start(sp, d, &["--plain"], &env);
    let code = wait(&mut c, Duration::from_secs(900));
    let a = alive(m);
    eprintln!(
        "G11-M1: session 2 exit {code}, marked {m} alive {a}; filtered listings {}",
        std::fs::read_to_string(&log)
            .map(|s| s.lines().count())
            .unwrap_or(0)
    );
    (code, a)
}

#[test]
#[ignore = "gate11 G11-M1: FAILS on fdec15da (exit 0 with the walk-blind marked process alive)"]
fn g11_walk_e2e_walk_blind_marked_survivor() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("g11-walk");
    let (Some(so), Some(thr)) = (cc(&t, "g11filter.c", true), cc(&t, "g11thr.c", false)) else {
        return;
    };
    let (sp, d, m, p) = crashed_session(&t, "thread", &thr);
    let leader =
        std::fs::read_to_string(format!("/proc/{p}/task/{p}/children")).unwrap_or_default();
    assert!(
        !leader.split_whitespace().any(|w| w == m.to_string()),
        "precondition: the marked {m} is not on its parent's leader list"
    );
    let (code, a) = filtered_session2(&t, &sp, &d, m, &so);
    kill_pid(m);
    if p > 1 {
        kill_pid(p);
    }
    assert!(
        !(code == 0 && a),
        "exit 0 with the crashed session's live marked process {m} (a child of a non-leader thread of {p}): {}",
        log_tail(&d)
    );
}

#[test]
fn g11_walk_e2e_reached_daemon_drop_is_caught() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("g11-walk-ctl");
    let (Some(so), Some(thr)) = (cc(&t, "g11filter.c", true), cc(&t, "g11thr.c", false)) else {
        return;
    };
    let (sp, d, m, _) = crashed_session(&t, "daemon", &thr);
    let (code, a) = filtered_session2(&t, &sp, &d, m, &so);
    kill_pid(m);
    assert!(
        !(code == 0 && a),
        "exit 0 with the crashed session's live marked daemon {m}: {}",
        log_tail(&d)
    );
    assert_ne!(
        code,
        0,
        "the incomplete listing must be named: {}",
        log_tail(&d)
    );
}
