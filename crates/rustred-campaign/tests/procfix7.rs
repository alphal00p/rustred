//! Lane procfix7 (the sixth gate audit, WS/notes/implementer.gate6.md). Spawns processes and sends signals: run ONLY
//! inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10); every test refuses to run outside one.
//! - MJ-A: a non-dumpable process of our uid that does not descend from the campaign never makes a sweep inconclusive
//!   (gate6 nd_inner.sh 'during' / 'resume'); one that may descend from it is never classified away and is NAMED.
//!   (The parent-chain classification on real processes: tests/procfix7_lineage.rs.) This file uses the CLI and the
//!   ledger only, so that it also runs against the sources before the fix.
//!
//! The capability rule of these tests: inside the namespace every uid-0 process holds all capabilities and can read a
//! non-dumpable process's environ (CAP_SYS_PTRACE); on the host a process of the user cannot. The supervisor and the
//! decoys therefore run with an EMPTY capability bounding set (as the user's processes on the host), and a decoy is made
//! non-dumpable the way the kernel does it for any exec of a file its caller cannot read (an execute-only copy of
//! `sleep`, exec'd by a capability-less shell).

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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

/// Empty the capability bounding set before the exec (procfix6's helper): the program runs as uid 0 of the namespace
/// WITHOUT any capability, like a process of the user on the host.
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

/// An execute-only copy of `sleep`: whoever execs it without CAP_DAC_OVERRIDE becomes NON-DUMPABLE (the kernel's rule
/// for an exec of an unreadable file). Its name stays `sleep` (coreutils may be one multicall binary that dispatches on
/// the basename of argv[0]).
fn xonly_sleep(dir: &Path) -> PathBuf {
    let d = dir.join("xonly");
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("sleep");
    std::fs::copy(sleep_bin(), &p).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o111)).unwrap();
    p
}

/// `rr_campaign run` as a capability-less process (host-like reader of other processes' environ).
fn start_capless(spec: &Path, cdir: &Path, extra: &[&str]) -> Child {
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
        .args(extra)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .env_remove("RR_CAMPAIGN_FAILPOINT");
    drop_caps(&mut c);
    c.spawn().unwrap()
}

/// The decoy `pid` is alive and non-dumpable, waiting up to 20 s: its pid file is written right after the fork, before
/// the child has exec'd the execute-only copy of sleep (under load that exec can lag behind the test).
fn non_dumpable_soon(pid: u32) -> bool {
    let t0 = Instant::now();
    loop {
        if alive(pid) && refused_to_capless(pid) {
            return true;
        }
        if t0.elapsed() > Duration::from_secs(20) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Is the environ of `pid` refused to a capability-less process of our uid (the host-like condition of the test)?
fn refused_to_capless(pid: u32) -> bool {
    let mut c = Command::new("sh");
    c.args(["-c", &format!("cat /proc/{pid}/environ > /dev/null 2>&1")])
        .env_clear()
        .env("PATH", path_env());
    drop_caps(&mut c);
    !c.status().unwrap().success()
}

/// Test code: SIGKILL one exact pid of ours (the shell's builtin; no signal call outside the guard in the crate).
fn kill_pid(pid: u32) {
    let _ = Command::new("sh")
        .args(["-c", &format!("kill -KILL {pid} 2>/dev/null")])
        .env("PATH", path_env())
        .status();
}

fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| s.rfind(')').map(|i| !s[i + 2..].starts_with('Z')))
        .unwrap_or(false)
}

/// A "tailscaled" stand-in: a capability-less shell started BEFORE the campaign (old) that, once `go` exists, starts
/// an UNMARKED non-dumpable child (the stand-in of the host's `be-child ssh`) and writes its pid to `nd.pid`.
fn spawner(dir: &Path) -> (Child, PathBuf, PathBuf) {
    let go = dir.join("go");
    let pidf = dir.join("nd.pid");
    let x = xonly_sleep(dir);
    let s = script(
        dir,
        "spawner.sh",
        &format!(
            "#!/bin/sh\nwhile [ ! -e {go} ]; do sleep 0.05; done\n{x} 600 </dev/null >/dev/null 2>&1 &\necho $! > {pidf}.tmp\n\
             mv {pidf}.tmp {pidf}\nwait\n",
            go = go.display(),
            x = x.display(),
            pidf = pidf.display()
        ),
    );
    let mut c = Command::new(&s);
    c.env_clear().env("PATH", path_env()).stdin(Stdio::null());
    drop_caps(&mut c);
    (c.spawn().unwrap(), go, pidf)
}

fn read_pid(p: &Path) -> u32 {
    wait_until("the non-dumpable decoy", Duration::from_secs(30), || {
        p.exists()
    });
    std::fs::read_to_string(p).unwrap().trim().parse().unwrap()
}

fn signalled_pids(d: &Path) -> Vec<u32> {
    records(d)
        .into_iter()
        .filter_map(|r| match r.event {
            Event::SignalSent { pid, .. } => Some(pid),
            _ => None,
        })
        .collect()
}

fn small_spec(t: &Tmp, seed: u64) -> PathBuf {
    let v = spec(
        t,
        serde_json::json!({"items": 8, "nodes": 2, "rungs": 1, "seed": seed, "case_ms": [400, 1200],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.25}),
        serde_json::json!({"max_jobs": 2}),
    );
    write_spec(t, "spec.json", &v)
}

/// gate6 MJ-A, nd_inner.sh 'during': an UNMARKED non-dumpable process of our uid, started while the campaign runs by
/// a process older than the campaign (the host's `tailscaled` -> `be-child ssh`), is proved not ours by its parent
/// chain: no sweep is inconclusive, the campaign ends as uninterrupted (before: every job-end sweep inconclusive, final
/// sweep unproven, exit 8 "not killed: []", 8 items in 7 min). The decoy is never signalled.
#[test]
fn mj_a_unrelated_non_dumpable_process_never_makes_sweeps_inconclusive() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-nd");
    let (mut sp, go, pidf) = spawner(&t.path);
    std::thread::sleep(Duration::from_millis(300)); // the spawner is older than every session (clock ticks)
    let sp_ = small_spec(&t, 9);
    let d = t.cdir("c");
    let t0 = Instant::now();
    let mut c = start_capless(&sp_, &d, &["--plain"]);
    std::thread::sleep(Duration::from_millis(1500));
    std::fs::write(&go, b"").unwrap();
    let nd = read_pid(&pidf);
    assert!(
        non_dumpable_soon(nd),
        "the decoy must be alive and non-dumpable for the test to mean anything"
    );
    let code = wait(&mut c, Duration::from_secs(240));
    let took = t0.elapsed();
    assert_eq!(code, finish_code_by_items(&d), "{}", log_tail(&d));
    assert_ne!(code, ATTENTION, "{}", log_tail(&d));
    assert!(
        notes(&d, "sweep_inconclusive").is_empty(),
        "{}",
        log_tail(&d)
    );
    assert!(
        notes(&d, "final_sweep_unproven").is_empty(),
        "{}",
        log_tail(&d)
    );
    assert!(notes(&d, "lineage_rule_off").is_empty(), "{}", log_tail(&d));
    assert_eq!(notes(&d, "session_lineage").len(), 1);
    assert!(took < Duration::from_secs(120), "took {took:?}");
    assert!(alive(nd), "the decoy must never be signalled");
    assert!(!signalled_pids(&d).contains(&nd));
    assert_eq!(verify(&d), 0);
    kill_pid(nd);
    let _ = sp.kill();
    let _ = sp.wait();
}

/// gate6 MJ-A, nd_inner.sh 'resume': session 1 crashes (SIGKILL), the unrelated non-dumpable process starts, session 2
/// resumes: the recovery sweep (bound = session 1's start; session 1's recorded ancestors are re-parent targets) and
/// every later sweep are conclusive; the run ends as uninterrupted (before: recovery 10790 rounds unproven, exit 8).
#[test]
fn mj_a_resume_with_an_unrelated_non_dumpable_process() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-ndr");
    let (mut sp, go, pidf) = spawner(&t.path);
    std::thread::sleep(Duration::from_millis(300));
    let sp_ = small_spec(&t, 10);
    let d = t.cdir("c");
    let mut c = start_capless(&sp_, &d, &["--plain"]);
    wait_until("two started jobs", Duration::from_secs(120), || {
        d.join("ledger").join("MANIFEST.json").exists()
            && count(&d, |e| {
                matches!(
                    e,
                    Event::Job {
                        ev: JobEvent::Started { .. },
                        ..
                    }
                )
            }) >= 2
    });
    signal(&c, libc::SIGKILL);
    let _ = c.wait();
    std::thread::sleep(Duration::from_millis(500));
    std::fs::write(&go, b"").unwrap();
    let nd = read_pid(&pidf);
    assert!(
        non_dumpable_soon(nd),
        "the decoy must be alive and non-dumpable"
    );
    let mut c2 = start_capless(&sp_, &d, &["--plain"]);
    let code = wait(&mut c2, Duration::from_secs(240));
    assert_eq!(code, finish_code_by_items(&d), "{}", log_tail(&d));
    assert!(
        notes(&d, "recovery_sweep_unproven").is_empty(),
        "{}",
        log_tail(&d)
    );
    assert!(
        notes(&d, "sweep_inconclusive").is_empty(),
        "{}",
        log_tail(&d)
    );
    assert!(
        notes(&d, "final_sweep_unproven").is_empty(),
        "{}",
        log_tail(&d)
    );
    assert_eq!(notes(&d, "session_lineage").len(), 2);
    assert!(alive(nd));
    assert!(!signalled_pids(&d).contains(&nd));
    assert_eq!(verify(&d), 0);
    kill_pid(nd);
    let _ = sp.kill();
    let _ = sp.wait();
}

/// MJ-A, the other side: a non-dumpable process that DESCENDS from a job (re-parented to the supervisor, the child
/// subreaper) carries the marker unreadably: it is never classified away. Every check that meets it stays
/// inconclusive and NAMES it ("descends from this supervisor"), nothing signals it, and an operator stop ends the run.
#[test]
fn mj_a_non_dumpable_descendant_of_a_job_is_named_and_never_proved_away() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-ndd");
    let x = xonly_sleep(&t.path);
    let pids = t.path.join("nd-desc.txt");
    let wrap = script(
        &t.path,
        "wrap.sh",
        &format!(
            "#!/bin/sh\nif [ \"$1\" = job ] && [ ! -e {pids} ]; then\n  setsid {x} 600 </dev/null >/dev/null 2>&1 &\n  \
             echo $! > {pids}\nfi\nexec {fake} \"$@\"\n",
            x = x.display(),
            pids = pids.display(),
            fake = FAKE
        ),
    );
    let mut v = spec(
        &t,
        serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "seed": 3, "case_ms": [300, 500],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0}),
        serde_json::json!({"max_jobs": 1}),
    );
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    let sp_ = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let mut c = start_capless(&sp_, &d, &["--plain", "--fake-child"]);
    let nd = read_pid(&pids);
    assert!(
        non_dumpable_soon(nd),
        "the decoy must be alive and non-dumpable"
    );
    let named = |d: &Path| {
        std::fs::read_to_string(d.with_extension("log"))
            .unwrap_or_default()
            .lines()
            .any(|l| {
                l.contains(&format!("pid {nd} ")) && l.contains("descends from this supervisor")
            })
    };
    wait_until(
        "a check naming the non-dumpable descendant",
        Duration::from_secs(180),
        || named(&d),
    );
    let o = Command::new(SUP)
        .args(["control", d.to_str().unwrap(), "stop"])
        .output()
        .unwrap();
    assert!(o.status.success());
    let code = wait(&mut c, Duration::from_secs(300));
    assert!(matches!(code, 140 | ATTENTION), "{code}: {}", log_tail(&d));
    assert!(alive(nd), "never signalled");
    assert!(!signalled_pids(&d).contains(&nd));
    assert!(
        notes(&d, "final_sweep_unproven")
            .iter()
            .all(|n| !n.contains("\"pids\":[]") || n.contains(&nd.to_string())),
        "an unproven end names the process"
    );
    // gate7 MJ-1: the operator's stop NAMES what holds the unproven attempt
    let log = std::fs::read_to_string(d.with_extension("log")).unwrap_or_default();
    assert!(
        log.lines()
            .any(|l| l.contains("end still unproven") && l.contains(&format!("pid {nd} "))),
        "the stop names pid {nd}: {}",
        log_tail(&d)
    );
    assert!(
        notes(&d, "unproven_at_exit")
            .iter()
            .any(|n| n.contains(&nd.to_string()))
    );
    kill_pid(nd);
}

/// gate7 MJ-1: a run whose attempt is held by a LIVE NON-DUMPABLE marked descendant ends ON ITS OWN: after
/// `resources.unproven_end_limit_s` (here 5 s) the supervisor names every process that may hold the attempt (note
/// `unproven_end`, the pid of the descendant), releases the attempt's CPU lease (the attempt keeps its job slot and
/// reservation: with max_jobs 1 nothing new launches) and stops with exit 8 naming it (before: the attempt stayed in the
/// launcher quarantine forever, lease held); after the operator kills that pid, the same command ends the campaign.
#[test]
fn mj1_an_unproven_end_is_named_its_lease_released_and_the_run_ends_8() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-mj1");
    let x = xonly_sleep(&t.path);
    let pids = t.path.join("nd-desc.txt");
    let wrap = script(
        &t.path,
        "wrap.sh",
        &format!(
            "#!/bin/sh\nif [ \"$1\" = job ] && [ ! -e {pids} ]; then\n  setsid {x} 600 </dev/null >/dev/null 2>&1 &\n  \
             echo $! > {pids}\nfi\nexec {fake} \"$@\"\n",
            x = x.display(),
            pids = pids.display(),
            fake = FAKE
        ),
    );
    let mut v = spec(
        &t,
        serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "seed": 3, "case_ms": [300, 500],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0}),
        serde_json::json!({"max_jobs": 1, "unproven_end_limit_s": 5}),
    );
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    let sp_ = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let mut c = start_capless(&sp_, &d, &["--plain", "--fake-child"]);
    let nd = read_pid(&pids);
    assert!(
        non_dumpable_soon(nd),
        "the decoy must be alive and non-dumpable"
    );
    let t0 = Instant::now();
    let code = loop {
        if let Some(st) = c.try_wait().unwrap() {
            use std::os::unix::process::ExitStatusExt;
            break st.code().unwrap_or(128 + st.signal().unwrap_or(0));
        }
        if t0.elapsed() > Duration::from_secs(150) {
            let _ = c.kill();
            let _ = c.wait();
            kill_pid(nd);
            panic!("the run did not end on its own: {}", log_tail(&d));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(code, ATTENTION, "{}", log_tail(&d));
    let n = notes(&d, "unproven_end");
    assert!(n.iter().any(|x| x.contains(&nd.to_string())), "{n:?}");
    let log = std::fs::read_to_string(d.with_extension("log")).unwrap_or_default();
    assert!(
        log.lines()
            .any(|l| l.contains("operator decision needed") && l.contains(&nd.to_string())),
        "the exit-8 message names pid {nd}: {}",
        log_tail(&d)
    );
    // the CPU lease was released; the attempt keeps its job slot and reservation (the process may still run): with
    // max_jobs 1 nothing new launched, so the run ended within seconds of the limit
    assert!(
        log.lines()
            .any(|l| l.contains("end UNPROVEN after") && l.contains("CPU lease released")),
        "{}",
        log_tail(&d)
    );
    assert!(
        result(&d).accepted.is_empty(),
        "nothing new in the held slot: {:?}",
        result(&d)
    );
    assert!(alive(nd), "never signalled");
    assert!(!signalled_pids(&d).contains(&nd));
    // the operator kills exactly the named pid and runs the same command: the campaign ends normally
    kill_pid(nd);
    let codes = run_to_end(&sp_, &d, &["--plain", "--fake-child"]);
    assert_eq!(codes.last(), Some(&0), "{codes:?}: {}", log_tail(&d));
    assert_eq!(result(&d).accepted.len(), 2, "{:?}", result(&d));
}

/// gate7 MJ-2: session 1 runs in a TIME NAMESPACE (boottime offset +100000 s: its start times are shifted) and crashes
/// leaving a marked daemon; session 2 runs outside. The recorded time namespace differs: every bound is 0 and the
/// lineage rule is off, so the recovery sweep finds and kills the daemon and the campaign ends with the reference
/// (before: '0 marked process(es) seen', exit 0, the daemon alive).
#[test]
fn mj2_a_session_in_another_time_namespace_never_hides_a_marked_process() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-mj2");
    let base = spec(
        &t,
        serde_json::json!({"items": 4, "nodes": 1, "rungs": 1, "seed": 8, "case_ms": [2500, 4000],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 2}),
    );
    let r = t.cdir("ref");
    let spr = write_spec(&t, "spec_ref.json", &base);
    let ref_codes = run_to_end(&spr, &r, &[]);
    let reference = result(&r);
    let mut v = base.clone();
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(daemon_wrapper_once(&t.path));
    let sp_ = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(d.with_extension("log"))
        .unwrap();
    let mut s1 = Command::new("unshare")
        .args(["-T", "--boottime", "100000", "--fork", "--kill-child", "--"])
        .arg(SUP)
        .args(["run", "--spec"])
        .arg(&sp_)
        .arg("--dir")
        .arg(&d)
        .env("PATH", path_env())
        .env_remove("RR_CAMPAIGN_FAILPOINT")
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    wait_until(
        "two started jobs and the daemon",
        Duration::from_secs(120),
        || started_count(&d) >= 2 && t.path.join("daemon.txt").exists(),
    );
    // the daemon's start time as THIS process reads it (the recorded one is shifted by the time namespace)
    let dm = {
        let pid = daemon_of(&t.path).0;
        (pid, start_ticks(pid))
    };
    // the unshare parent dies, --kill-child SIGKILLs the supervisor (a crash)
    let _ = s1.kill();
    let _ = s1.wait();
    std::thread::sleep(Duration::from_millis(500));
    assert!(alive_with(dm.0, dm.1));
    let codes = run_to_end(&sp_, &d, &[]);
    assert_eq!(
        codes.last(),
        ref_codes.last(),
        "{codes:?}: {}",
        log_tail(&d)
    );
    assert!(
        !notes(&d, "time_namespace_differs").is_empty(),
        "{}",
        log_tail(&d)
    );
    assert!(
        !alive_with(dm.0, dm.1),
        "the marked daemon of the time-namespace session survived"
    );
    assert_eq!(result(&d), reference);
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

/// gate7 m-nr: on a file system without renameat2 RENAME_NOREPLACE (test build: `run_dir_noreplace:1`) no run
/// directory can be created: the run stops with exit 1 after a bounded number of tries naming the condition (before:
/// the same WARN forever, never an end).
#[test]
fn m_nr_run_dirs_that_cannot_be_created_end_typed() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-mnr");
    let sp_ = small_spec(&t, 5);
    let d = t.cdir("c");
    let mut c = start(
        &sp_,
        &d,
        &[],
        &[("RR_CAMPAIGN_FAILPOINT", "run_dir_noreplace:1")],
    );
    let code = wait(&mut c, Duration::from_secs(180));
    assert_eq!(code, 1, "{}", log_tail(&d));
    assert!(
        log_tail(&d).contains("RENAME_NOREPLACE"),
        "{}",
        log_tail(&d)
    );
    assert_eq!(launch_count(&d), 0, "nothing launched");
    assert_eq!(verify(&d), 0);
}

/// gate7 m-rw: the external work directory is moved away while a job runs: the run ends with exit 70 naming the work
/// directory (before: exit 1 'not this campaign's', blaming another campaign, and the directory re-created without its
/// claim). Moved back, the same command finishes with the reference result.
#[test]
fn m_rw_a_removed_external_work_dir_is_a_typed_end_with_a_true_message() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-mrw");
    let work = t.path.join("work");
    let base = spec(
        &t,
        serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "seed": 7, "case_ms": [3000, 4000],
            "p_norule": 0.0, "p_mem": 0.0, "p_crash": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 1}),
    );
    let mut v = base.clone();
    v["dirs"] = serde_json::json!({"work_dir": work});
    let sp_ = write_spec(&t, "spec.json", &v);
    let spr = write_spec(&t, "spec_ref.json", &base);
    let rd = t.cdir("ref");
    let ref_codes = run_to_end(&spr, &rd, &[]);
    let reference = result(&rd);
    let d = t.cdir("c");
    let mut c = start(&sp_, &d, &[], &[]);
    wait_until("a started job", Duration::from_secs(120), || {
        started_count(&d) >= 1
    });
    let aside = t.path.join("work.moved");
    std::fs::rename(&work, &aside).unwrap();
    let code = wait(&mut c, Duration::from_secs(300));
    assert_eq!(code, 70, "{}", log_tail(&d));
    assert!(
        log_tail(&d).contains("external work directory"),
        "{}",
        log_tail(&d)
    );
    assert!(
        !log_tail(&d).contains("belongs to campaign"),
        "{}",
        log_tail(&d)
    );
    assert!(
        !work.exists(),
        "the work directory is never re-created (unclaimed)"
    );
    std::fs::rename(&aside, &work).unwrap();
    let codes = run_to_end(&sp_, &d, &[]);
    assert_eq!(
        codes.last(),
        ref_codes.last(),
        "{codes:?}: {}",
        log_tail(&d)
    );
    assert_eq!(result(&d), reference);
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

fn started_count(d: &Path) -> usize {
    if !d.join("ledger").join("MANIFEST.json").exists() {
        return 0;
    }
    count(d, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Started { .. },
                ..
            }
        )
    })
}

fn launch_count(d: &Path) -> usize {
    count(d, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Launch { .. },
                ..
            }
        )
    })
}

/// Every directory below `root` whose name says it was moved aside ("unrecorded").
fn moved_aside(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                if e.file_name().to_string_lossy().contains(".unrecorded-") {
                    out.push(p.clone());
                }
                stack.push(p);
            }
        }
    }
    out
}

/// gate6 MJ-B (cp_inner.sh 'shared'): the original crashes with 3 jobs running, its directory is copied, and the copy
/// (`--adopt-moved-campaign`) and the original's restart run AT ONCE with ONE external `dirs.work_dir`. The copy is
/// REFUSED (exit 1) before it touches anything (no launch, no rename, no read); the original finishes with the result
/// of an uninterrupted reference; no directory is renamed anywhere (before: both rc 10, results != reference, 7 run
/// directories renamed).
#[test]
fn mj_b_copy_sharing_an_external_work_dir_is_refused_and_the_original_finishes() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-cp");
    let work = t.path.join("work");
    let base = spec(
        &t,
        serde_json::json!({"items": 6, "nodes": 2, "rungs": 1, "seed": 5, "case_ms": [3000, 5000],
            "p_norule": 0.25, "p_mem": 0.0, "p_crash": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 3}),
    );
    let mut v = base.clone();
    v["dirs"] = serde_json::json!({"work_dir": work});
    let sp_ = write_spec(&t, "spec.json", &v);
    let spr = write_spec(&t, "spec_ref.json", &base);
    let rd = t.cdir("ref");
    let ref_codes = run_to_end(&spr, &rd, &[]);
    let reference = result(&rd);
    let c = t.cdir("c");
    let mut o = start(&sp_, &c, &[], &[]);
    wait_until("three started jobs", Duration::from_secs(120), || {
        started_count(&c) >= 3
    });
    std::thread::sleep(Duration::from_secs(1));
    signal(&o, libc::SIGKILL);
    let _ = o.wait();
    assert!(work.join("RR_CAMPAIGN_WORK_OWNER").exists());
    let c2 = t.cdir("c2");
    let st = Command::new("cp")
        .arg("-a")
        .arg(&c)
        .arg(&c2)
        .env("PATH", path_env())
        .status()
        .unwrap();
    assert!(st.success());
    let launches_before = launch_count(&c2);
    let mut nc = start(&sp_, &c2, &[], &[]);
    assert_eq!(
        wait(&mut nc, Duration::from_secs(60)),
        7,
        "a copy without --adopt-moved-campaign"
    );
    let mut cc = start(&sp_, &c2, &["--adopt-moved-campaign"], &[]);
    let mut o2 = start(&sp_, &c, &[], &[]);
    let ccode = wait(&mut cc, Duration::from_secs(120));
    assert_eq!(ccode, 1, "the copy must be refused: {}", log_tail(&c2));
    assert!(
        log_tail(&c2).contains("belongs to campaign"),
        "{}",
        log_tail(&c2)
    );
    let ocode = wait(&mut o2, Duration::from_secs(600));
    assert_eq!(Some(&ocode), ref_codes.last(), "{}", log_tail(&c));
    assert_eq!(result(&c), reference, "{}", log_tail(&c));
    assert_eq!(
        launch_count(&c2),
        launches_before,
        "the copy launched nothing"
    );
    assert!(
        moved_aside(&t.path).is_empty(),
        "{:?}",
        moved_aside(&t.path)
    );
    assert!(notes(&c, "run_dir_not_ours").is_empty());
    assert_eq!(verify(&c), 0);
    assert_no_marked(&t.campaign);
}

/// MJ-B, the settle side: a running attempt's run directory whose identity is replaced by another campaign's is NEVER
/// read at settle: the attempt is lost and requeued, the run drains and ends with the typed refusal (exit 1) naming
/// the directory; the restart redoes the work in a directory of its own and ends with the reference result.
#[test]
fn mj_b_settle_reads_nothing_from_a_run_dir_that_is_not_ours() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-ids");
    let work = t.path.join("work");
    let base = spec(
        &t,
        serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "seed": 7, "case_ms": [3000, 4000],
            "p_norule": 0.0, "p_mem": 0.0, "p_crash": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 1}),
    );
    let mut v = base.clone();
    v["dirs"] = serde_json::json!({"work_dir": work});
    let sp_ = write_spec(&t, "spec.json", &v);
    let spr = write_spec(&t, "spec_ref.json", &base);
    let rd = t.cdir("ref");
    let ref_codes = run_to_end(&spr, &rd, &[]);
    let reference = result(&rd);
    let c = t.cdir("c");
    let mut s1 = start(&sp_, &c, &[], &[]);
    wait_until("a started job", Duration::from_secs(120), || {
        started_count(&c) >= 1
    });
    let run_dir = records(&c)
        .into_iter()
        .find_map(|r| match r.event {
            Event::Job {
                ev: JobEvent::Launch { run_dir, .. },
                ..
            } => Some(PathBuf::from(run_dir)),
            _ => None,
        })
        .unwrap();
    let id_file = run_dir.join("sup").join("CAMPAIGN");
    std::fs::write(&id_file, "intruder.0123456789abcdef\n").unwrap();
    let code = wait(&mut s1, Duration::from_secs(300));
    assert_eq!(code, 1, "{}", log_tail(&c));
    let n = notes(&c, "run_dir_not_ours");
    assert!(
        !n.is_empty() && n[0].contains(&run_dir.display().to_string()),
        "{n:?}"
    );
    let lost = count(
        &c,
        |e| matches!(e, Event::Job { ev: JobEvent::Lose { reason }, .. } if reason.contains("not this campaign's")),
    );
    assert_eq!(lost, 1, "{}", log_tail(&c));
    assert!(run_dir.exists(), "never renamed or removed");
    let codes = run_to_end(&sp_, &c, &[]);
    assert_eq!(
        codes.last(),
        ref_codes.last(),
        "{codes:?}: {}",
        log_tail(&c)
    );
    assert_eq!(result(&c), reference);
    assert_eq!(verify(&c), 0);
    assert_no_marked(&t.campaign);
}

/// gate6 m-x: a failpoint value without `:n` used to be ignored silently (a void test looked like a pass): the test
/// build now refuses to start (exit 1) naming the part, before it creates a ledger.
#[test]
fn m_x_failpoint_without_count_is_refused() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-fp");
    let sp_ = small_spec(&t, 2);
    let d = t.cdir("c");
    let mut c = start(
        &sp_,
        &d,
        &[],
        &[("RR_CAMPAIGN_FAILPOINT", "no_intent_prealloc")],
    );
    assert_eq!(wait(&mut c, Duration::from_secs(60)), 1, "{}", log_tail(&d));
    let log = std::fs::read_to_string(d.with_extension("log")).unwrap_or_default();
    assert!(log.contains("not <site>:<n>"), "{log}");
    assert!(!d.join("ledger").join("MANIFEST.json").exists());
}

/// gate6 m2-a: 32 garbage bytes written into a waiting shim's barrier pipe (here by the test, which holds
/// CAP_SYS_PTRACE in its namespace; on the host: a same-uid process in the instant between the shim's exec and its
/// prctl) make the shim refuse the release: the program never runs, and the attempt is a TYPED end (row 15s, requeued
/// after the back-off), never a crash: no crash record, no item in attention, exit 0 and the reference result (before:
/// row 23 crash, attention, exit 10). The test build holds each release 1.5 s (`release_delay_ms`) so the pipe can be
/// reached deterministically.
#[test]
fn m2a_garbage_in_the_barrier_pipe_is_a_typed_retry() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-m2a");
    let v = spec(
        &t,
        serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "seed": 6, "case_ms": [300, 600],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0}),
        serde_json::json!({"max_jobs": 1}),
    );
    let sp_ = write_spec(&t, "spec.json", &v);
    let r = t.cdir("ref");
    let ref_codes = run_to_end(&sp_, &r, &[]);
    let d = t.cdir("c");
    let mut c = start(
        &sp_,
        &d,
        &[],
        &[("RR_CAMPAIGN_FAILPOINT", "release_delay_ms:1500")],
    );
    // the first shim waiting behind its barrier: its argv names the barrier descriptor
    let (shim, fd) = {
        let t0 = Instant::now();
        loop {
            let found = std::fs::read_dir("/proc").unwrap().flatten().find_map(|e| {
                let pid: u32 = e.file_name().to_str()?.parse().ok()?;
                let argv = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
                let a: Vec<String> = argv
                    .split(|&b| b == 0)
                    .map(|x| String::from_utf8_lossy(x).into_owned())
                    .collect();
                if a.get(1).map(String::as_str) != Some("__spawn-barrier") {
                    return None;
                }
                let fd: i32 = a.get(2)?.parse().ok()?;
                Some((pid, fd))
            });
            if let Some(x) = found {
                break x;
            }
            assert!(t0.elapsed() < Duration::from_secs(60), "{}", log_tail(&d));
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .open(format!("/proc/{shim}/fd/{fd}"))
            .expect("the test holds CAP_SYS_PTRACE in its namespace");
        f.write_all(b"GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG").unwrap();
    }
    let code = wait(&mut c, Duration::from_secs(300));
    assert_eq!(Some(&code), ref_codes.last(), "{}", log_tail(&d));
    let crashed = count(&d, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Finished {
                    class: rustred_campaign::state::FinishClass::Crashed,
                    ..
                },
                ..
            }
        )
    });
    assert_eq!(crashed, 0, "{}", log_tail(&d));
    let typed = count(
        &d,
        |e| matches!(e, Event::Job { ev: JobEvent::Requeue { reason }, .. } if reason.ends_with("(row 15s)")),
    );
    assert_eq!(typed, 1, "{}", log_tail(&d));
    assert_eq!(result(&d), result(&r));
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

/// gate6 m3-a: on ZFS nothing in the campaign directory reserves room for the signal intent log; the tmpfs FALLBACK
/// does. Here the campaign directory's log is full (test build: `intent_primary_enospc:1`) and the supervisor crashes
/// right after its first send (`signal_after_send:1`): the intent is in the tmpfs fallback (never in the campaign
/// directory's log), the signal WAS sent, and the restart records it from the fallback (`signal_intents_reconciled`)
/// and then removes the earlier session's fallback file. The fallback lives on the test binary's private /dev/shm.
#[test]
fn m3a_full_intent_log_uses_the_tmpfs_fallback_and_the_restart_reads_it() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-m3a");
    let v = spec(
        &t,
        serde_json::json!({"items": 1, "nodes": 1, "rungs": 1, "deps": 0.0, "case_ms": [600000, 600000],
            "seed": 9, "p_mem": 0.0, "p_crash": 0.0, "p_norule": 0.0}),
        serde_json::json!({"grace_seconds": 2}),
    );
    let sp_ = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let mut c = start(
        &sp_,
        &d,
        &[],
        &[(
            "RR_CAMPAIGN_FAILPOINT",
            "intent_primary_enospc:1,signal_after_send:1",
        )],
    );
    wait_until("a started job", Duration::from_secs(120), || {
        started_count(&d) >= 1
    });
    std::thread::sleep(Duration::from_secs(1));
    signal(&c, libc::SIGINT);
    let code = wait(&mut c, Duration::from_secs(120));
    assert_eq!(
        code,
        134,
        "the abort right after the first send: {}",
        log_tail(&d)
    );
    let primary =
        std::fs::read_to_string(d.join("log").join("signal-intents.jsonl")).unwrap_or_default();
    assert!(!primary.contains("\"kind\":\"intent\""), "{primary}");
    let fb_dir = rustred_campaign::launch::intent_fallback_dir(&d).unwrap();
    assert!(fb_dir.starts_with("/dev/shm"), "{}", fb_dir.display());
    let fb: Vec<PathBuf> = std::fs::read_dir(&fb_dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    let intents: usize = fb
        .iter()
        .map(|p| {
            std::fs::read_to_string(p)
                .unwrap_or_default()
                .matches("\"kind\":\"intent\"")
                .count()
        })
        .sum();
    assert!(intents >= 1, "the intent is in the tmpfs fallback: {fb:?}");
    // the restart (no failpoint) records the dead session's signal from the fallback
    let mut c2 = start(&sp_, &d, &[], &[]);
    wait_until("the reconciled intent", Duration::from_secs(120), || {
        d.join("ledger").join("MANIFEST.json").exists()
            && !notes(&d, "signal_intents_reconciled").is_empty()
    });
    let o = Command::new(SUP)
        .args(["control", d.to_str().unwrap(), "stop"])
        .output()
        .unwrap();
    assert!(o.status.success());
    assert_eq!(
        wait(&mut c2, Duration::from_secs(120)),
        140,
        "{}",
        log_tail(&d)
    );
    let n = notes(&d, "signal_intents_reconciled");
    assert!(
        n.iter()
            .any(|t| !t.contains("\"delivered_unrecorded\":0,\"possibly_sent\":0")),
        "{n:?}"
    );
    for p in &fb {
        assert!(
            !p.exists(),
            "the earlier session's fallback is removed after the merge: {}",
            p.display()
        );
    }
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

/// A job wrapper whose FIRST job leaves a marked daemon (double fork, own session; it carries the job's marker) and
/// records `pid start` in `daemon.txt`.
fn daemon_wrapper_once(dir: &Path) -> PathBuf {
    let rec = dir.join("daemon.txt");
    script(
        dir,
        "wrap-daemon.sh",
        &format!(
            "#!/bin/sh\nif [ \"$1\" = job ] && [ ! -e {rec} ]; then\n  ( setsid {sleep} 600 </dev/null >/dev/null 2>&1 & \
             d=$!; echo \"$d $(cut -d' ' -f22 /proc/$d/stat)\" > {rec}.tmp; mv {rec}.tmp {rec} )\nfi\nexec {fake} \"$@\"\n",
            rec = rec.display(),
            sleep = sleep_bin().display(),
            fake = FAKE
        ),
    )
}

fn daemon_of(dir: &Path) -> (u32, u64) {
    let t = std::fs::read_to_string(dir.join("daemon.txt")).unwrap();
    let mut it = t.split_whitespace();
    (
        it.next().unwrap().parse().unwrap(),
        it.next().unwrap().parse().unwrap(),
    )
}

fn alive_with(pid: u32, start: u64) -> bool {
    alive(pid) && start_ticks(pid) == start
}

/// Session 1 of the B1 scenario: jobs run, the first leaves a marked daemon, the supervisor is SIGKILLed with two jobs
/// started (their attempts unsettled). Returns (spec, cdir, daemon, reference result, reference exit code).
fn b1_setup(
    t: &Tmp,
) -> (
    PathBuf,
    PathBuf,
    (u32, u64),
    rustred_campaign::supervisor::result::FinalResult,
    i32,
) {
    let base = spec(
        t,
        serde_json::json!({"items": 4, "nodes": 1, "rungs": 1, "seed": 8, "case_ms": [2500, 4000],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 2}),
    );
    let r = t.cdir("ref");
    let spr = write_spec(t, "spec_ref.json", &base);
    let ref_codes = run_to_end(&spr, &r, &[]);
    let reference = result(&r);
    let mut v = base.clone();
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(daemon_wrapper_once(&t.path));
    let sp_ = write_spec(t, "spec.json", &v);
    let d = t.cdir("c");
    let mut s1 = start(&sp_, &d, &[], &[]);
    wait_until(
        "two started jobs and the daemon",
        Duration::from_secs(120),
        || started_count(&d) >= 2 && t.path.join("daemon.txt").exists(),
    );
    signal(&s1, libc::SIGKILL);
    let _ = s1.wait();
    let dm = daemon_of(&t.path);
    assert!(alive_with(dm.0, dm.1));
    (sp_, d, dm, reference, *ref_codes.last().unwrap())
}

/// gate7 B1: a read of RECOVERY that fails once (test build: `recovery_read_fails:<n>` = the n-th read, for every n
/// until a session meets no fault) stops the start TYPED (exit 71) and decides nothing: when the ledger / intent-log
/// pre-read fails, no record about any attempt is written and nothing is signalled (the marked daemon of the crashed
/// session lives); when a run-directory read fails, the attempt read is not decided. The first session without a fault
/// recovers: the daemon is killed by the recovery sweep and the campaign ends with the reference result (before: one
/// failed ledger read gave no recovery sweep, the final sweep bounded by its own start, and exit 0 with the daemon
/// alive).
#[test]
fn b1_a_failed_recovery_read_stops_typed_and_decides_nothing() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-b1");
    let (sp_, d, dm, reference, ref_code) = b1_setup(&t);
    let mut faulted = 0usize;
    let mut pre_read = 0usize;
    let mut last = None;
    for n in 1..=40usize {
        let before = records(&d).len();
        let fp = format!("recovery_read_fails:{n}");
        let mut c = start(&sp_, &d, &[], &[("RR_CAMPAIGN_FAILPOINT", fp.as_str())]);
        let code = wait(&mut c, Duration::from_secs(600));
        if code != 71 {
            last = Some(code);
            break;
        }
        faulted += 1;
        let log = std::fs::read_to_string(d.join("log/supervisor.log")).unwrap_or_default();
        let line = log
            .lines()
            .rev()
            .find(|l| l.contains("recovery cannot read"))
            .unwrap_or_else(|| {
                panic!(
                    "n={n}: exit 71 without the recovery-read line: {}",
                    log_tail(&d)
                )
            })
            .to_string();
        assert!(line.contains("nothing was decided"), "{line}");
        let new = &records(&d)[before..];
        if line.contains("cannot read the ledger") || line.contains("signal intent log") {
            // the pre-read: nothing about any attempt, nothing signalled
            pre_read += 1;
            assert!(
                !new.iter().any(|r| matches!(
                    r.event,
                    Event::Job { .. }
                        | Event::Item { .. }
                        | Event::SignalSent { .. }
                        | Event::OrphanKilled { .. }
                )),
                "n={n}: a decision after a failed pre-read: {:?}",
                new.iter().map(|r| r.event.kind()).collect::<Vec<_>>()
            );
            assert!(
                alive_with(dm.0, dm.1),
                "n={n}: the daemon was signalled after a failed pre-read"
            );
        } else {
            // a run-directory read: the attempt named is not decided by this session
            let who = line
                .split("the run directory of ")
                .nth(1)
                .or_else(|| line.split("job.json of ").nth(1))
                .or_else(|| line.split("the spawn sentinel of ").nth(1))
                .unwrap_or_else(|| panic!("n={n}: which read? {line}"));
            let mut it = who.split_whitespace();
            let job = it.next().unwrap().to_string();
            let attempt: u32 = it.next().unwrap().trim_start_matches('a').parse().unwrap();
            assert!(
                !new.iter().any(|r| matches!(&r.event, Event::Job { job: j, attempt: a, .. } if *j == job && *a == attempt)),
                "n={n}: {job} a{attempt} was decided after its read failed"
            );
        }
    }
    eprintln!("b1: {faulted} faulted session(s) ({pre_read} in the pre-read), then exit {last:?}");
    assert!(
        faulted >= 3,
        "only {faulted} faulted session(s): the reads of recovery are not all instrumented"
    );
    assert!(
        pre_read >= 2,
        "the ledger pass and the intent log read must both be instrumented ({pre_read})"
    );
    // the first session without a fault ran the campaign: the reference, the daemon killed by the recovery sweep
    let mut codes: Vec<i32> = last.into_iter().collect();
    if !codes.last().is_some_and(|c| finished(*c)) {
        codes.extend(run_to_end(&sp_, &d, &[]));
    }
    assert_eq!(codes.last().copied(), Some(ref_code), "{}", log_tail(&d));
    assert_eq!(result(&d), reference, "{}", log_tail(&d));
    assert!(
        !alive_with(dm.0, dm.1),
        "the crashed session's marked daemon survived"
    );
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

/// gate7 B1, the auditor's scenario on the real binary: an LD_PRELOAD shim fails the SECOND open of
/// ledger/MANIFEST.json (the recovery pass) with EMFILE: the start stops with exit 71 (before: exit 0 with the crashed
/// session's marked daemon alive); the next start recovers to the reference. Needs `cc`; ignored by default.
#[test]
#[ignore = "builds an LD_PRELOAD shim with cc; run explicitly"]
fn b1_preload_fault_on_the_second_manifest_open() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-b1p");
    let c_src = t.path.join("fault.c");
    std::fs::write(
        &c_src,
        r#"#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
static int count = 0;
static int fail(const char *p) {
  const char *s = getenv("PF7_FAIL_SUFFIX"), *at = getenv("PF7_FAIL_AT");
  if (!s || !at || !p) return 0;
  size_t lp = strlen(p), ls = strlen(s);
  if (lp < ls || strcmp(p + lp - ls, s)) return 0;
  return __atomic_add_fetch(&count, 1, __ATOMIC_SEQ_CST) == atoi(at);
}
#define W(name) int name(const char *p, int f, ...) { mode_t m = 0; if (f & (O_CREAT | O_TMPFILE)) { va_list a; \
  va_start(a, f); m = va_arg(a, int); va_end(a); } if (fail(p)) { errno = EMFILE; return -1; } \
  static int (*r)(const char *, int, ...) = 0; if (!r) r = dlsym(RTLD_NEXT, #name); return r(p, f, m); }
W(open) W(open64)
#define WA(name) int name(int d, const char *p, int f, ...) { mode_t m = 0; if (f & (O_CREAT | O_TMPFILE)) { va_list a; \
  va_start(a, f); m = va_arg(a, int); va_end(a); } if (fail(p)) { errno = EMFILE; return -1; } \
  static int (*r)(int, const char *, int, ...) = 0; if (!r) r = dlsym(RTLD_NEXT, #name); return r(d, p, f, m); }
WA(openat) WA(openat64)
"#,
    )
    .unwrap();
    let so = t.path.join("fault.so");
    let built = Command::new("cc")
        .args(["-shared", "-fPIC", "-O1", "-o"])
        .arg(&so)
        .arg(&c_src)
        .arg("-ldl")
        .env("PATH", path_env())
        .status();
    if !built.is_ok_and(|s| s.success()) {
        eprintln!("SKIPPED: no C compiler for the preload shim");
        return;
    }
    let (sp_, d, dm, reference, ref_code) = b1_setup(&t);
    let so_s = so.to_str().unwrap().to_string();
    let mut c = start(
        &sp_,
        &d,
        &[],
        &[
            ("LD_PRELOAD", so_s.as_str()),
            ("PF7_FAIL_SUFFIX", "ledger/MANIFEST.json"),
            ("PF7_FAIL_AT", "2"),
        ],
    );
    let code = wait(&mut c, Duration::from_secs(600));
    assert_eq!(code, 71, "{}", log_tail(&d));
    assert!(
        alive_with(dm.0, dm.1),
        "nothing may be signalled after the failed read"
    );
    let codes = run_to_end(&sp_, &d, &[]);
    assert_eq!(codes.last().copied(), Some(ref_code), "{}", log_tail(&d));
    assert_eq!(result(&d), reference);
    assert!(
        !alive_with(dm.0, dm.1),
        "the crashed session's marked daemon survived"
    );
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

/// gate7 B1, the class: a signal intent log that exists but cannot be read (here an earlier session's tmpfs fallback,
/// mode 000, read by a capability-less supervisor) is never ignored: the run cannot claim a clean end: exit 8 with the
/// ledger note `end_unproven` naming the file, and the file is kept (not removed with the readable ones).
#[test]
fn an_unreadable_intent_log_never_gives_a_clean_end() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-unr");
    let sp_ = small_spec(&t, 4);
    let d = t.cdir("c");
    std::fs::create_dir_all(&d).unwrap();
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let fb_dir = rustred_campaign::launch::intent_fallback_dir(&d).unwrap();
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&fb_dir)
        .unwrap();
    let old = fb_dir.join("earlier.jsonl");
    std::fs::write(&old, b"").unwrap();
    std::fs::set_permissions(&old, std::fs::Permissions::from_mode(0o000)).unwrap();
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(d.with_extension("log"))
        .unwrap();
    let mut c = Command::new(SUP);
    c.arg("run")
        .arg("--spec")
        .arg(&sp_)
        .arg("--dir")
        .arg(&d)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .env_remove("RR_CAMPAIGN_FAILPOINT");
    drop_caps(&mut c);
    let mut c = c.spawn().unwrap();
    let code = wait(&mut c, Duration::from_secs(300));
    assert_eq!(code, ATTENTION, "{}", log_tail(&d));
    let n = notes(&d, "end_unproven");
    assert!(n.iter().any(|x| x.contains("earlier.jsonl")), "{n:?}");
    assert!(old.exists(), "an unreadable log is never removed");
    assert_eq!(verify(&d), 0);
}
