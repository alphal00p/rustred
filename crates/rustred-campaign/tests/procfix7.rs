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
        alive(nd) && refused_to_capless(nd),
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
    assert!(alive(nd) && refused_to_capless(nd));
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
    assert!(alive(nd) && refused_to_capless(nd));
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
    kill_pid(nd);
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
