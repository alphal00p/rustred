//! gate9 audit (WS/notes/implementer.gate9.md), BLOCKER G9-B1: a STAT ERROR of `ledger/MANIFEST.json` is taken as "no
//! ledger". `supervisor::start` (`manifest.exists()`) then CREATES a new ledger over the crashed campaign:
//! `Ledger::create_with` checks `exists()` again (same mapping) and its residue check lists the directory with
//! `read_dir(..)?.flatten()`, which silently drops a readdir error (an empty listing = "no residue"); the removal loop
//! deletes the commit mark and segment 1; the new ledger records only this session, so the recovery and final sweeps
//! start at this session's start and the crashed session's marked daemon is outside the bound: FINISHED, exit 0, the
//! daemon alive, the history gone.
//!
//! Ignored (needs a C compiler: the gate9 LD_PRELOAD injector `gate9_g9fault.c`, a copy of
//! implementer-scratch/gate9/src/g9fault.c): FAILS on 9876cba2. Run explicitly, ONLY inside a private pid namespace:
//! `LIGHT_PIDNS=1 light_build.sh gate9 test -p rustred-campaign --test gate9_ledger_create -- --ignored`.

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

fn supervisor_sessions(cdir: &Path) -> usize {
    count(cdir, |e| {
        matches!(e, rustred_campaign::ledger::Event::SupervisorStarted { .. })
    })
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

/// G9-B1: two failed stats of `ledger/MANIFEST.json` and one failed readdir of the ledger directory (EIO, then a fresh
/// campaign with ENOMEM) must never give exit 0 with the crashed session's live marked daemon, and must never replace
/// the campaign's history (session 1's `SupervisorStarted` stays recorded).
#[test]
#[ignore = "builds the gate9 LD_PRELOAD injector with cc; FAILS on 9876cba2 (G9-B1); run explicitly"]
fn g9_b1_unreadable_manifest_never_creates_a_new_ledger_over_a_campaign() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/gate9_g9fault.c");
    let probe = Tmp::new("g9-cc");
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
    for (tag, errno) in [("g9-mf-eio", "5"), ("g9-mf-enomem", "12")] {
        let t = Tmp::new(tag);
        let (sp, d, dp) = crashed_session_with_daemon(&t);
        assert_eq!(
            supervisor_sessions(&d),
            1,
            "precondition: session 1 recorded"
        );
        let m = d.join("ledger");
        let m_s = m.to_str().unwrap().to_string();
        let log = t.path.join("fault.log");
        let log_s = log.to_str().unwrap().to_string();
        let path = path_env();
        let env = [
            ("LD_PRELOAD", so_s.as_str()),
            ("G8_LOG", log_s.as_str()),
            ("G8_MATCH", m_s.as_str()),
            ("G8_OPS", "stat,readdir"),
            // stat #1 = supervisor `manifest.exists()`, stat #2 = `create_with`'s `exists()`, #3 the directory, #4
            // create_dir_all's is_dir, readdir #5 = the residue listing (dropped by `.flatten()`)
            ("G8_AT", "1,2,5"),
            ("G8_ERRNO", errno),
            ("PATH", path.to_str().unwrap()),
        ];
        let mut c = start(&sp, &d, &["--plain"], &env);
        let code = wait(&mut c, Duration::from_secs(600));
        let failed = std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter(|l| l.ends_with("FAILED"))
            .count();
        assert!(
            failed >= 2,
            "precondition: the injector failed the manifest stats"
        );
        let survivor = alive(dp);
        let sessions = supervisor_sessions(&d);
        kill_pid(dp);
        assert!(
            !(code == 0 && survivor),
            "errno {errno}: exit 0 with the crashed session's live marked daemon {dp}: {}",
            log_tail(&d)
        );
        assert!(
            sessions >= 2,
            "errno {errno}: the campaign's history was replaced by a new ledger (SupervisorStarted records: {sessions}; exit {code}): {}",
            log_tail(&d)
        );
    }
}
