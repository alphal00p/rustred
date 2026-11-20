//! Lane procfix7 round 4 (gate8 G8-B1, WS/notes/implementer.gate8.md): exit 0 with the crashed session's LIVE MARKED
//! non-dumpable daemon when reads of its `/proc/<pid>/stat` fail. Before: `lineage::stat_at` mapped every failed read to
//! "no stat", the parent-chain walk answered Gone for the candidate itself, the strict scan dropped it, and the recovery
//! and final sweeps were "proven" (FINISHED, exit 0, the daemon alive). After: only ENOENT / ESRCH is Gone
//! (`resources::procfs`); the candidate stays possibly ours and the run ends with exit 8 naming it.
//!
//! Spawns processes and sends signals: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule
//! L10). Host-like reading as in tests/procfix7.rs: every supervisor and job runs with an EMPTY capability bounding set,
//! and the daemon is made non-dumpable by an exec of an execute-only copy of `sleep`.
//! - in the normal suite: the in-process fault injector of `procfs` (test builds: `RR_CAMPAIGN_FAILPOINT=
//!   proc_stat_<errno>:2,proc_pid:<daemon>`: every 2nd read of the daemon's stat fails), EMFILE and EIO;
//! - ignored (needs a C compiler): gate8's own LD_PRELOAD injector `g8fault.c` (copied verbatim from
//!   implementer-scratch/gate8/src/g8fault.c), the gate's repro settings (G8_MATCH=/proc/<daemon>/stat G8_OPS=open
//!   G8_PERTHREAD=1 G8_FROM=2 G8_EVERY=2 G8_ERRNO=24 / 5).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
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

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// Empty the capability bounding set before the exec: the program runs as uid 0 of the namespace WITHOUT any
/// capability, like a process of the user on the host.
fn drop_caps(c: &mut Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: prctl is async-signal-safe; integers only.
    unsafe {
        c.pre_exec(|| {
            for cap in 0..64 {
                libc::prctl(libc::PR_CAPBSET_DROP, cap, 0, 0, 0);
            }
            Ok(())
        });
    }
}

/// An execute-only copy of `sleep` (an exec of it by a capability-less process is NON-DUMPABLE); its name stays
/// `sleep` (coreutils may be one multicall binary).
fn xonly_sleep(dir: &Path) -> PathBuf {
    let d = dir.join("xonly");
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("sleep");
    std::fs::copy(sleep_bin(), &p).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o111)).unwrap();
    p
}

/// `rr_campaign run` as a capability-less process, with extra environment (the failpoint is passed explicitly).
fn start_capless_env(spec: &Path, cdir: &Path, env: &[(&str, &str)]) -> Child {
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cdir.with_extension("log"))
        .unwrap();
    let mut c = Command::new(SUP);
    c.arg("run")
        .arg("--spec")
        .arg(spec)
        .arg("--dir")
        .arg(cdir)
        .arg("--plain")
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .env_remove("RR_CAMPAIGN_FAILPOINT");
    for (k, v) in env {
        c.env(k, v);
    }
    drop_caps(&mut c);
    c.spawn().unwrap()
}

fn refused_to_capless(pid: u32) -> bool {
    let mut c = Command::new("sh");
    c.args(["-c", &format!("cat /proc/{pid}/environ > /dev/null 2>&1")])
        .env_clear()
        .env("PATH", path_env());
    drop_caps(&mut c);
    !c.status().unwrap().success()
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

/// Session 1 of the gate8 scenario (g8_inj_inner.sh): a capability-less supervisor whose first job starts a MARKED
/// NON-DUMPABLE daemon (setsid, the execute-only sleep: it keeps the job's environment and marker); the supervisor is
/// SIGKILLed once the daemon runs (the daemon is re-parented to the namespace's init). Returns (spec, cdir, daemon).
fn crashed_session_with_daemon(t: &Tmp) -> (PathBuf, PathBuf, u32) {
    let x = xonly_sleep(&t.path);
    let rec = t.path.join("daemon.pid");
    let wrap = script(
        &t.path,
        "wrap.sh",
        &format!(
            "#!/bin/sh\nif [ \"$1\" = job ] && [ ! -e {rec} ]; then\n  ( setsid {x} 600 </dev/null >/dev/null 2>&1 & \
             echo $! > {rec}.tmp; mv {rec}.tmp {rec} )\nfi\nexec {fake} \"$@\"\n",
            rec = rec.display(),
            x = x.display(),
            fake = FAKE
        ),
    );
    let mut v = spec(
        t,
        serde_json::json!({"items": 4, "nodes": 1, "rungs": 1, "seed": 5, "case_ms": [1500, 2500],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        // procfix7 round 20 (gate R-C, the full procfix set): session 2's jobs can never prove their end while every 2nd
        // stat read of the hidden daemon fails (the hunt completes no round): the end is UNPROVEN after the verify
        // timeout (120 s) plus resources.unproven_end_limit_s (default 600 s) before exit 8, which this test's 600-s
        // bound could never see. The scenario is unchanged; only the limit is short.
        serde_json::json!({"max_jobs": 2, "unproven_end_limit_s": 20}),
    );
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    let sp = write_spec(t, "spec.json", &v);
    let d = t.cdir("c");
    let mut s1 = start_capless_env(&sp, &d, &[]);
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
    wait_until(
        "the daemon alive and non-dumpable",
        Duration::from_secs(20),
        || alive(dp) && refused_to_capless(dp),
    );
    (sp, d, dp)
}

/// Session 2 under the given environment: never exit 0 while the daemon lives; exit 8 naming it.
fn session_two_names_the_daemon(sp: &Path, d: &Path, dp: u32, env: &[(&str, &str)], what: &str) {
    let mut c = start_capless_env(sp, d, env);
    let code = wait(&mut c, Duration::from_secs(900));
    assert!(
        alive(dp),
        "{what}: nothing may signal the unreadable daemon"
    );
    assert_ne!(
        code,
        0,
        "{what}: exit 0 with the crashed session's live marked daemon {dp}: {}",
        log_tail(d)
    );
    assert_eq!(code, ATTENTION, "{what}: {}", log_tail(d));
    let log = std::fs::read_to_string(d.with_extension("log")).unwrap_or_default();
    assert!(
        log.lines().any(
            |l| (l.contains("not proven") || l.contains("operator decision"))
                && l.contains(&format!("pid {dp} "))
        ),
        "{what}: the exit-8 message names the daemon {dp}: {}",
        log_tail(d)
    );
}

/// gate8 G8-B1, in-process equivalent (normal suite): every 2nd read of the daemon's stat fails (EMFILE, then a fresh
/// campaign with EIO); session 2 ends with exit 8 naming the daemon, never exit 0.
#[test]
fn g8_b1_failed_stat_reads_of_a_marked_non_dumpable_daemon_never_end_0() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    for (tag, mode) in [("pf8-b1e", "emfile"), ("pf8-b1i", "eio")] {
        let t = Tmp::new(tag);
        let (sp, d, dp) = crashed_session_with_daemon(&t);
        let fp = format!("proc_stat_{mode}:2,proc_pid:{dp}");
        session_two_names_the_daemon(&sp, &d, dp, &[("RR_CAMPAIGN_FAILPOINT", &fp)], &fp);
        kill_pid(dp);
    }
}

/// gate8 G8-B1 with gate8's own LD_PRELOAD injector (`g8fault.c`, verbatim) and the gate's settings, EMFILE and EIO.
#[test]
#[ignore = "builds gate8's LD_PRELOAD injector with cc; run explicitly"]
fn g8_b1_gate8_preload_injector_never_gives_exit_0() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/procfix8_g8fault.c");
    let probe = Tmp::new("pf8-g8cc");
    let so = probe.path.join("g8fault.so");
    let built = Command::new("cc")
        .args(["-shared", "-fPIC", "-O1", "-o"])
        .arg(&so)
        .arg(&src)
        .arg("-ldl")
        .arg("-lpthread")
        .env("PATH", path_env())
        .status();
    if !built.is_ok_and(|s| s.success()) {
        eprintln!("SKIPPED: no C compiler for gate8's injector");
        return;
    }
    let so_s = so.to_str().unwrap().to_string();
    for (tag, errno) in [("pf8-g8e", "24"), ("pf8-g8i", "5")] {
        let t = Tmp::new(tag);
        let (sp, d, dp) = crashed_session_with_daemon(&t);
        let m = format!("/proc/{dp}/stat");
        let log = t.path.join("fault.log");
        let log_s = log.to_str().unwrap().to_string();
        let env = [
            ("LD_PRELOAD", so_s.as_str()),
            ("G8_LOG", log_s.as_str()),
            ("G8_MATCH", m.as_str()),
            ("G8_OPS", "open"),
            ("G8_PERTHREAD", "1"),
            ("G8_FROM", "2"),
            ("G8_EVERY", "2"),
            ("G8_ERRNO", errno),
        ];
        session_two_names_the_daemon(&sp, &d, dp, &env, &format!("g8fault errno {errno}"));
        let failed = std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter(|l| l.ends_with("FAILED"))
            .count();
        assert!(
            failed > 0,
            "the injector never failed a read (precondition)"
        );
        kill_pid(dp);
    }
}

/// gate8 m8-mut2: the check for hidden descendants that cannot RUN (its /proc reads fail; test build:
/// `hidden_descendants_unreadable:1`) is never a clean end: exit 8 with the ledger note `hidden_descendants_unchecked`
/// (a mutation that made the failed check pass went unnoticed by every test before).
#[test]
fn m8_mut2_an_unchecked_hidden_descendant_check_is_never_a_clean_end() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf8-hdu");
    let v = spec(
        &t,
        serde_json::json!({"items": 3, "nodes": 1, "rungs": 1, "seed": 4, "case_ms": [100, 300],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 2}),
    );
    let sp = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let mut c = start(
        &sp,
        &d,
        &["--plain"],
        &[("RR_CAMPAIGN_FAILPOINT", "hidden_descendants_unreadable:1")],
    );
    let code = wait(&mut c, Duration::from_secs(300));
    assert_eq!(code, ATTENTION, "{}", log_tail(&d));
    assert!(
        !notes(&d, "hidden_descendants_unchecked").is_empty(),
        "{}",
        log_tail(&d)
    );
    // without the failpoint the same campaign directory ends normally
    let codes = run_to_end(&sp, &d, &["--plain"]);
    assert!(
        finished(*codes.last().unwrap()),
        "{codes:?}: {}",
        log_tail(&d)
    );
}

/// gate8 m8-mut1 (gate7 m-xdg): the fallback directory of the intent logs is used only when it is a real directory owned
/// by us and closed to group / others: a base made world-accessible, a key directory with mode 0755, and a key directory
/// that is a symlink are each REFUSED (a mutation that dropped the mode check went unnoticed by every test before).
/// Runs on the test namespace's PRIVATE /dev/shm (sup_common::private_shm), never on the host's.
#[test]
fn m8_mut1_the_fallback_directory_must_be_private() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf8-xdg");
    use rustred_campaign::launch::{
        intent_fallback_base, intent_fallback_dir_checked, intent_fallback_logs,
    };
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let cdir = |n: &str| {
        let p = t.path.join(n);
        std::fs::create_dir_all(&p).unwrap();
        p
    };
    let key = |c: &Path| {
        let m = std::fs::metadata(c).unwrap();
        intent_fallback_base().join(format!("{}-{}", m.dev(), m.ino()))
    };
    let chmod = |p: &Path, mode: u32| {
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap()
    };
    // a private base and key directory: accepted (created 0700)
    let c1 = cdir("c1");
    let d1 = intent_fallback_dir_checked(&c1, true).unwrap();
    assert_eq!(
        std::fs::metadata(&d1).unwrap().permissions().mode() & 0o777,
        0o700
    );
    // the base made accessible to group / others: refused (create and list)
    let base = intent_fallback_base();
    chmod(&base, 0o777);
    let e = intent_fallback_dir_checked(&cdir("c2"), true).unwrap_err();
    assert!(
        e.to_string().contains("accessible to group / others"),
        "{e}"
    );
    assert!(
        intent_fallback_logs(&c1).is_err(),
        "a public base is never listed"
    );
    chmod(&base, 0o700);
    // a key directory with mode 0755: refused
    let c3 = cdir("c3");
    std::fs::DirBuilder::new()
        .mode(0o755)
        .create(key(&c3))
        .unwrap();
    chmod(&key(&c3), 0o755);
    let e = intent_fallback_dir_checked(&c3, true).unwrap_err();
    assert!(
        e.to_string().contains("accessible to group / others"),
        "{e}"
    );
    // a key directory that is a symlink (to a private directory): refused
    let c4 = cdir("c4");
    let target = t.path.join("elsewhere");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&target)
        .unwrap();
    std::os::unix::fs::symlink(&target, key(&c4)).unwrap();
    let e = intent_fallback_dir_checked(&c4, true).unwrap_err();
    assert!(e.to_string().contains("symlink"), "{e}");
    // the private one still works
    assert!(intent_fallback_dir_checked(&c1, false).is_ok());
}
