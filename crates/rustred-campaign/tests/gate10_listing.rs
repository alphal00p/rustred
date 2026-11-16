//! gate10 audit (WS/notes/implementer.gate10.md), BLOCKER G10-B1 (gate9's m9-ns, now end to end): a /proc listing that BOTH
//! enumerations (std readdir and raw getdents64: the same kernel getdents64 underneath) return truncated in agreement is
//! accepted as COMPLETE (resources/procfs.rs numeric_listing: only a DISAGREEMENT is Unreadable; nothing checks that the
//! listing contains a process known to be alive, e.g. the supervisor itself). An EMPTY /proc then means "no process": the
//! lock take-over, the recovery sweep and the final sweep see no marked process, the own-tree proof (waitid) concerns this
//! session only, and the crashed session's marked daemon survives an exit 0.
//!
//! Ignored (needs a C compiler: tests/gate9_g9fault.c, G8_ERRNO=trunc = readdir / getdents64 end early without an error):
//! FAILS on 3550b2ac (4 of 5 runs of the gate10 matrix probe S/cells_NS.txt: exit 0 with the daemon alive). Run explicitly,
//! ONLY inside a private pid namespace: `LIGHT_PIDNS=1 light_build.sh gate10 test -p rustred-campaign --test gate10_listing
//! -- --ignored`.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;
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

/// Session 1: its first job starts a MARKED daemon (setsid sleep keeps the job's environment); SIGKILLed once the daemon
/// runs. Returns (spec, cdir, daemon pid).
fn crashed_session_with_daemon(t: &Tmp) -> (PathBuf, PathBuf, u32) {
    let rec = t.path.join("daemon.pid");
    let wrap = t.path.join("wrap.sh");
    std::fs::write(
        &wrap,
        format!(
            "#!/bin/sh\nif [ \"$1\" = job ] && [ ! -e {rec} ]; then\n  ( setsid sleep 600 </dev/null >/dev/null 2>&1 & \
             echo $! > {rec}.tmp; mv {rec}.tmp {rec} )\nfi\nexec {fake} \"$@\"\n",
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
        serde_json::json!({"max_jobs": 2}),
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
    wait_until("the marked daemon", Duration::from_secs(60), || {
        rec.exists()
    });
    std::thread::sleep(Duration::from_millis(500));
    signal(&s1, libc::SIGKILL);
    let _ = s1.wait();
    let dp: u32 = std::fs::read_to_string(&rec)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(alive(dp), "precondition: the daemon lives");
    (sp, d, dp)
}

/// G10-B1: session 2 of a crashed campaign whose /proc listings (both enumerations) end early from the n-th listing call
/// on: never exit 0 with the crashed session's live marked daemon.
#[test]
#[ignore = "builds the gate9 LD_PRELOAD injector with cc; FAILS on 3550b2ac (G10-B1); run explicitly"]
fn g10_b1_truncated_proc_listing_never_gives_exit_0_with_a_marked_survivor() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/gate9_g9fault.c");
    let probe = Tmp::new("g10-cc");
    let so = probe.path.join("g9fault.so");
    let built = Command::new("cc")
        .args(["-shared", "-fPIC", "-O1", "-o"])
        .arg(&so)
        .arg(&src)
        .arg("-ldl")
        .arg("-lpthread")
        .env("PATH", path_env())
        .status();
    if !built.is_ok_and(|s| s.success()) {
        eprintln!("SKIPPED: no C compiler for the gate9 injector");
        return;
    }
    let so_s = so.to_str().unwrap().to_string();
    let mut failures = Vec::new();
    for from in ["1", "100"] {
        let t = Tmp::new(&format!("g10-trunc-{from}"));
        let (sp, d, dp) = crashed_session_with_daemon(&t);
        let log = t.path.join("fault.log");
        let log_s = log.to_str().unwrap().to_string();
        let path = path_env();
        let env = [
            ("LD_PRELOAD", so_s.as_str()),
            ("G8_LOG", log_s.as_str()),
            ("G8_MATCH", "/proc"),
            ("G8_OPS", "readdir,getdents"),
            ("G8_FROM", from),
            ("G8_ERRNO", "trunc"),
            ("PATH", path.to_str().unwrap()),
        ];
        let mut c = start(&sp, &d, &["--plain"], &env);
        let code = wait(&mut c, Duration::from_secs(900));
        let survivor = alive(dp);
        kill_pid(dp);
        eprintln!("G10-B1 trunc from listing call {from}: exit {code}, daemon alive {survivor}");
        if code == 0 && survivor {
            failures.push(format!(
                "from {from}: exit 0 with the crashed session's live marked daemon {dp}: {}",
                log_tail(&d)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
