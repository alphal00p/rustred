//! Lane int5: regressions of the independent gate audit gate3 (WS/notes/implementer.gate3.md), at the supervisor level
//! with real `rr_campaign` + `rr_fake_child` (run with LIGHT_PIDNS=1, rule L10; every planted process is a child of this
//! test inside its private pid namespace):
//! - M1 / M2: a process carrying THIS campaign's exact marker is ours wherever its cwd is (double-forked, setsid'd,
//!   chdir "/"): killed through the guard and recorded (`orphan_killed`, `signal_sent`); decoys (another campaign's id,
//!   no marker, this campaign's marker but started BEFORE the campaign / the attempt) are never signalled;
//! - M1: the final sweep never ends in exit 0 when it is not proven (exit 8, pids in the ledger);
//! - M2 / m2: an exact-marker process in the supervisor's own session is killed by the end-of-job hunt and recorded
//!   (no 120 s spin), every delivery is a `signal_sent` record, the launch record names the scope unit (M5);
//! - M3 / M4: under low process limits the campaign is bounded (no job-id runaway) and never ends in exit 0 with a
//!   result other than the reference; a resumed run equals the reference;
//! - M3: a helper thread that cannot be started is a typed resource failure, not a timeout;
//! - m3: the site's CPU-ledger and lock files are never created, and bad site configurations are refused.

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use rustred_campaign::state::AttemptState;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A `sleep 600` with `RR_CAMPAIGN_JOB=<marker>` (or no marker), in `cwd`, optionally in a new session (a double-forked
/// daemon), stdin / stdout / stderr closed. A child of this test (inside the namespace).
fn plant(marker: Option<&str>, cwd: &Path, setsid: bool) -> Child {
    let mut c = Command::new(sleep_bin());
    c.arg("600")
        .current_dir(cwd)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(m) = marker {
        c.env("RR_CAMPAIGN_JOB", m);
    }
    if setsid {
        // SAFETY: setsid is async-signal-safe; no allocation in the closure.
        unsafe {
            c.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    c.spawn().unwrap()
}

fn killed_by_sigkill(c: &mut Child) -> bool {
    use std::os::unix::process::ExitStatusExt;
    let t0 = Instant::now();
    loop {
        if let Some(s) = c.try_wait().unwrap() {
            return s.signal() == Some(libc::SIGKILL);
        }
        if t0.elapsed() > Duration::from_secs(30) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn signal_records(d: &Path) -> Vec<(u32, i32, String, String)> {
    records(d)
        .into_iter()
        .filter_map(|r| match r.event {
            Event::SignalSent {
                pid,
                signal,
                by,
                proof,
                ..
            } => Some((pid, signal, by, proof)),
            _ => None,
        })
        .collect()
}

/// A campaign directory whose marker id is known BEFORE its first supervisor (the nonce is written by the test).
fn precreated(t: &Tmp, name: &str) -> (PathBuf, String) {
    let d = t.cdir(name);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("CAMPAIGN_NONCE"),
        "0123456789abcdef0123456789abcdef\n",
    )
    .unwrap();
    let id = rustred_campaign::supervisor::identity::campaign_marker_id(&d, &t.campaign).unwrap();
    (d, id)
}

fn running_attempt(d: &Path) -> Option<String> {
    rustred_campaign::ledger::open_read_only(d)
        .ok()?
        .state
        .jobs
        .iter()
        .find(|(_, j)| j.state == AttemptState::Running)
        .map(|(id, j)| format!("{id}:a{}", j.attempt))
}

/// gate3 M1 + M2 decoys. After a supervisor CRASH, two processes carrying this campaign's exact marker (a daemon of a
/// running attempt: setsid'd, cwd "/", no fds; a ghost attempt in TMPDIR) are killed by the restart's sweep and
/// recorded; the campaign ends with exit 0 equal to the reference. Decoys survive and are never in a `signal_sent`
/// record: this campaign's exact marker started BEFORE the campaign, another campaign's id (same name, other nonce /
/// directory), no marker.
#[test]
fn marked_processes_outside_the_campaign_dirs_are_killed_recorded_and_decoys_survive() {
    let _g = serial();
    let t = Tmp::new("g3-m1");
    let syn = serde_json::json!({"items": 8, "case_ms": [3000, 5000], "seed": 5, "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let r = t.cdir("reference");
    assert_eq!(run_to_end(&spec_p, &r, &[]), vec![0]);
    let reference = result(&r);
    let j0 = state(&r).jobs.keys().next().cloned().unwrap();

    let (d, id) = precreated(&t, "run");
    // decoy 1: this campaign's EXACT marker, started before any supervisor of this campaign
    let mut early = plant(Some(&format!("{id}:{j0}:a1")), &d, true);
    std::thread::sleep(Duration::from_millis(200));
    let mut c = start(&spec_p, &d, &[], &[]);
    wait_until("a running attempt", Duration::from_secs(120), || {
        d.join("ledger").join("MANIFEST.json").exists() && running_attempt(&d).is_some()
    });
    let l = running_attempt(&d).unwrap();
    let tmp = std::env::temp_dir();
    let mut daemon = plant(Some(&format!("{id}:{l}")), Path::new("/"), true);
    let mut ghost = plant(Some(&format!("{id}:ghost-job:a1")), &tmp, true);
    // decoys 2, 3: another campaign's id (same name, another nonce / directory), no marker
    let mut other = plant(
        Some(&format!("{}.0000000000000000:{l}", t.campaign)),
        &d,
        true,
    );
    let mut plain = plant(None, &d, true);
    std::thread::sleep(Duration::from_millis(500));
    signal(&c, libc::SIGKILL);
    assert_eq!(wait(&mut c, Duration::from_secs(60)), 137);
    let codes = run_to_end(&spec_p, &d, &[]);
    assert_eq!(
        codes,
        vec![0],
        "no operator decision, no idling: {}",
        log_tail(&d)
    );
    assert_eq!(result(&d), reference);
    assert_eq!(verify(&d), 0);
    // ours: killed (SIGKILL through the guard) and recorded twice (orphan_killed, signal_sent)
    let sigs = signal_records(&d);
    for (what, p) in [
        ("daemon (cwd /)", &mut daemon),
        ("ghost (TMPDIR)", &mut ghost),
    ] {
        let pid = p.id();
        assert!(killed_by_sigkill(p), "{what} {pid} not killed by SIGKILL");
        assert!(
            count(
                &d,
                |e| matches!(e, Event::OrphanKilled { pid: q, .. } if *q == pid)
            ) >= 1,
            "{what}: orphan_killed record"
        );
        assert!(
            sigs.iter()
                .any(|(q, s, _, _)| *q == pid && *s == libc::SIGKILL),
            "{what}: signal_sent record; {sigs:?}"
        );
    }
    assert!(notes(&d, "marked_survivor").is_empty());
    // decoys: alive, never signalled
    for (what, p) in [
        ("early exact marker", &mut early),
        ("other campaign id", &mut other),
        ("no marker", &mut plain),
    ] {
        assert!(p.try_wait().unwrap().is_none(), "decoy {what} was harmed");
        assert!(
            !sigs.iter().any(|(q, ..)| *q == p.id()),
            "decoy {what} appears in a signal_sent record"
        );
    }
    for p in [&mut early, &mut other, &mut plain] {
        let _ = p.kill();
        let _ = p.wait();
    }
    assert_no_marked(&t.campaign);
}

/// gate3 M1: an unproven final sweep never ends in exit 0. Test-build failpoint `final_sweep_fails`: the final sweep
/// reports itself unproven -> exit 8, ledger note `final_sweep_unproven`, no `campaign finish`; the next run (without
/// the failpoint) finishes with exit 0 equal to the reference.
#[test]
fn unproven_final_sweep_never_exits_0() {
    let _g = serial();
    let t = Tmp::new("g3-final");
    let syn = serde_json::json!({"items": 4, "case_ms": [100, 300], "seed": 3, "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let r = t.cdir("reference");
    assert_eq!(run_to_end(&spec_p, &r, &[]), vec![0]);
    let reference = result(&r);
    let d = t.cdir("run");
    let mut c = start(
        &spec_p,
        &d,
        &[],
        &[("RR_CAMPAIGN_FAILPOINT", "final_sweep_fails:1")],
    );
    assert_eq!(
        wait(&mut c, Duration::from_secs(300)),
        ATTENTION,
        "{}",
        log_tail(&d)
    );
    assert_eq!(notes(&d, "final_sweep_unproven").len(), 1);
    assert!(
        count(&d, |e| matches!(
            e,
            Event::Campaign {
                ev: rustred_campaign::state::CampaignEvent::Finish,
                ..
            }
        )) == 0,
        "never finished while the final sweep is unproven"
    );
    let full = std::fs::read_to_string(d.with_extension("log")).unwrap();
    assert!(
        full.contains("final marker sweep not proven"),
        "{}",
        log_tail(&d)
    );
    let mut c = start(&spec_p, &d, &[], &[]);
    assert_eq!(
        wait(&mut c, Duration::from_secs(300)),
        0,
        "{}",
        log_tail(&d)
    );
    assert_eq!(result(&d), reference);
    assert_eq!(verify(&d), 0);
}

/// gate3 M2 / m2 (hunt decoy): a process NOT descended from the campaign, in the supervisor's OWN session, carrying a
/// running attempt's exact marker, is ours by the one rule: the end-of-job hunt stops and kills it and the delivery is
/// a `signal_sent` record (before: never signalled by the hunt, spun on for the 120 s budget, then killed by the job
/// signaller without any record). This campaign's exact marker started BEFORE the attempt / the campaign is never
/// signalled. Every `signal_sent` record names a pid that is not a decoy; the launch records name the scope unit
/// (M5).
#[test]
fn own_session_exact_marker_is_killed_by_the_hunt_and_recorded_early_decoy_survives() {
    let _g = serial();
    let t = Tmp::new("g3-hunt");
    let syn = serde_json::json!({"items": 6, "case_ms": [2500, 4000], "seed": 9, "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let r = t.cdir("reference");
    let t0 = Instant::now();
    assert_eq!(run_to_end(&spec_p, &r, &[]), vec![0]);
    let ref_secs = t0.elapsed().as_secs();
    let reference = result(&r);
    let j0 = state(&r).jobs.keys().next().cloned().unwrap();

    let (d, id) = precreated(&t, "run");
    let mut early = plant(Some(&format!("{id}:{j0}:a1")), &std::env::temp_dir(), false);
    std::thread::sleep(Duration::from_millis(200));
    let t1 = Instant::now();
    let mut c = start(&spec_p, &d, &[], &[]);
    wait_until("a running attempt", Duration::from_secs(120), || {
        d.join("ledger").join("MANIFEST.json").exists() && running_attempt(&d).is_some()
    });
    let l = running_attempt(&d).unwrap();
    // in THIS test's session (= the supervisor's session: it is our child without setsid), cwd "/"
    let mut late = plant(Some(&format!("{id}:{l}")), Path::new("/"), false);
    assert_eq!(
        wait(&mut c, Duration::from_secs(600)),
        0,
        "{}",
        log_tail(&d)
    );
    let secs = t1.elapsed().as_secs();
    assert_eq!(result(&d), reference);
    assert!(
        secs < ref_secs + 90,
        "no 120 s hunt spin: {secs} s vs reference {ref_secs} s"
    );
    let lpid = late.id();
    assert!(
        killed_by_sigkill(&mut late),
        "the own-session marked process {lpid}"
    );
    let sigs = signal_records(&d);
    assert!(
        sigs.iter().any(|(q, s, by, _)| *q == lpid
            && *s == libc::SIGKILL
            && (by.contains("hunt") || by.contains("sweep") || by.contains("job signal"))),
        "{sigs:?}"
    );
    assert!(early.try_wait().unwrap().is_none(), "early decoy harmed");
    assert!(!sigs.iter().any(|(q, ..)| *q == early.id()));
    // M5: every launch record names its deterministic scope unit
    for rec in records(&d) {
        if let Event::Job {
            job,
            attempt,
            ev: JobEvent::Launch { scope_unit, .. },
        } = &rec.event
        {
            assert_eq!(
                scope_unit.as_deref(),
                Some(
                    rustred_campaign::launch::scope_unit_name(&id, &format!("{job}:a{attempt}"))
                        .as_str()
                )
            );
        }
    }
    let _ = early.kill();
    let _ = early.wait();
    assert_no_marked(&t.campaign);
}

/// gate3 M3 / M4 (gate letter c, process limits): the supervisor under RLIMIT_NPROC = u (inherited by its children)
/// for a range of u: bounded (it ends within the bound, no job-id runaway), its exit code is a typed one, an exit 0 has
/// the reference result (host-resource ends of children are never negative evidence), and the unlimited resume equals
/// the reference.
#[test]
fn low_process_limits_are_bounded_and_never_change_the_result() {
    let _g = serial();
    let t = Tmp::new("g3-nproc");
    let syn = serde_json::json!({"items": 10, "case_ms": [300, 700], "seed": 9, "p_mem": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let r = t.cdir("reference");
    // procfix5 (gate4 m3/m4): the reference may finish with 10 (a synthetic item that crashed twice is in attention)
    let ref_codes = run_to_end(&spec_p, &r, &[]);
    assert!(
        ref_codes.len() == 1 && finished(ref_codes[0]),
        "{ref_codes:?}"
    );
    let ref_code = ref_codes[0];
    let reference = result(&r);
    let jobs_ref = count(&r, |e| matches!(e, Event::JobQueued { .. }));
    let mut summary = Vec::new();
    for u in [5u64, 6, 7, 8, 9, 10, 12] {
        let d = t.cdir(&format!("nproc{u}"));
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(d.with_extension("log"))
            .unwrap();
        let mut cmd = Command::new(SUP);
        cmd.args(["run", "--spec"])
            .arg(&spec_p)
            .arg("--dir")
            .arg(&d)
            .stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .env_remove("RR_CAMPAIGN_FAILPOINT");
        // SAFETY: setrlimit is async-signal-safe; no allocation in the closure.
        unsafe {
            cmd.pre_exec(move || {
                let lim = libc::rlimit {
                    rlim_cur: u,
                    rlim_max: u,
                };
                libc::setrlimit(libc::RLIMIT_NPROC, &lim);
                Ok(())
            });
        }
        let mut c = cmd.spawn().unwrap();
        let t0 = Instant::now();
        let code = loop {
            if let Some(s) = c.try_wait().unwrap() {
                use std::os::unix::process::ExitStatusExt;
                break s.code().unwrap_or(128 + s.signal().unwrap_or(0));
            }
            if t0.elapsed() > Duration::from_secs(240) {
                signal(&c, libc::SIGINT);
                let _ = wait(&mut c, Duration::from_secs(60));
                panic!(
                    "nproc {u}: not bounded (no exit within 240 s): {}",
                    log_tail(&d)
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let minted = if d.join("ledger").join("MANIFEST.json").exists() {
            count(&d, |e| matches!(e, Event::JobQueued { .. }))
        } else {
            0
        };
        summary.push(format!(
            "nproc {u}: exit {code}, {minted} job ids (reference {jobs_ref})"
        ));
        assert!(
            matches!(code, 0 | 8 | 10 | 71 | 75 | 1),
            "nproc {u}: untyped exit {code}: {}",
            log_tail(&d)
        );
        assert!(
            minted <= jobs_ref * 2 + 12,
            "nproc {u}: job-id runaway ({minted} vs {jobs_ref}): {}",
            log_tail(&d)
        );
        if finished(code) {
            assert_eq!(code, ref_code, "nproc {u}: finished with another code");
            assert_eq!(
                result(&d),
                reference,
                "nproc {u}: exit {code} with another result"
            );
        }
        if d.join("ledger").join("MANIFEST.json").exists() {
            let codes = run_to_end(&spec_p, &d, &[]);
            assert_eq!(codes.last(), Some(&ref_code), "nproc {u} resume: {codes:?}");
            assert_eq!(result(&d), reference, "nproc {u}: resumed result");
            assert_eq!(verify(&d), 0);
        }
    }
    eprintln!("{}", summary.join("\n"));
    assert_no_marked(&t.campaign);
}

/// gate3 M3: a helper thread that cannot be started is a typed resource failure (EAGAIN), never a timeout.
#[test]
fn helper_thread_that_cannot_start_is_a_typed_resource_failure() {
    use rustred_campaign::resources::{Bounded, MAX_STUCK, run_bounded};
    let _g = serial();
    let stuck: Vec<_> = (0..MAX_STUCK)
        .map(|_| {
            run_bounded(Duration::from_millis(1), || {
                std::thread::sleep(Duration::from_millis(1500));
            })
        })
        .collect();
    assert!(stuck.iter().all(|b| matches!(b, Bounded::TimedOut)));
    match run_bounded(Duration::from_secs(1), || 1) {
        Bounded::SpawnFailed(e) => assert_eq!(e.raw_os_error(), Some(libc::EAGAIN), "{e}"),
        other => panic!("expected SpawnFailed, got {other:?}"),
    }
    std::thread::sleep(Duration::from_millis(2000));
    assert!(matches!(
        run_bounded(Duration::from_secs(5), || 2),
        Bounded::Done(2)
    ));
}

/// gate3 m3: the site's files are never created by the campaign, and a bad site configuration is refused at once
/// (lock name escaping the lock directory, missing lock directory / ledger, a FIFO, a directory or a file inside the
/// lock directory as ledger); a CPU without its lock file is never granted.
#[test]
fn site_files_are_never_created_and_bad_site_configs_are_refused() {
    use rustred_campaign::cpu_policy::{CpuPolicy, SITE_SCHEMA, SiteConfig, SitePolicy};
    let _g = serial();
    let t = Tmp::new("g3-site");
    let root = t.path.join("site");
    let locks = root.join("locks");
    std::fs::create_dir_all(&locks).unwrap();
    let ledger = root.join("CPU_LEDGER.md");
    let cpu = rustred_campaign::resources::allowed_cpus()
        .unwrap()
        .into_iter()
        .find(|c| !(168..=175).contains(c) && !(288..=319).contains(c))
        .unwrap();
    let cfg = |lock_name: &str, ledger_file: &Path, lock_dir: &Path| SiteConfig {
        schema: SITE_SCHEMA.into(),
        tag: "rr-int5-test".into(),
        lock_dir: lock_dir.to_path_buf(),
        lock_name: lock_name.into(),
        ledger_file: ledger_file.to_path_buf(),
        ledger_scan_lines: 100,
        candidates: Some(cpu.to_string()),
        exclude: vec![],
        held_files: vec![],
        pause_files: vec![],
        idle_min: 0.0,
        max_cores: 1,
        respect_foreign_pinning: false,
        ignore_own_tag_claims: true,
    };
    // the ledger does not exist: refused, not created
    assert!(SitePolicy::new(cfg("cpu.{n}", &ledger, &locks)).is_err());
    assert!(!ledger.exists());
    // the lock directory does not exist: refused, not created
    std::fs::write(&ledger, b"").unwrap();
    let nolocks = root.join("no-locks");
    assert!(SitePolicy::new(cfg("cpu.{n}", &ledger, &nolocks)).is_err());
    assert!(!nolocks.exists());
    // a lock name escaping the lock directory
    for bad in ["../escape.{n}", "a/cpu.{n}", "..{n}"] {
        assert!(SitePolicy::new(cfg(bad, &ledger, &locks)).is_err(), "{bad}");
    }
    assert!(!root.join(format!("escape.{cpu}")).exists());
    // a FIFO, a directory, a file inside the lock directory as ledger: refused at once (never blocks)
    let fifo = root.join("fifo");
    let cp = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: a valid C string and mode.
    assert_eq!(unsafe { libc::mkfifo(cp.as_ptr(), 0o600) }, 0);
    let t0 = Instant::now();
    assert!(SitePolicy::new(cfg("cpu.{n}", &fifo, &locks)).is_err());
    assert!(SitePolicy::new(cfg("cpu.{n}", &root, &locks)).is_err());
    let inside = locks.join("ledger.md");
    std::fs::write(&inside, b"").unwrap();
    assert!(SitePolicy::new(cfg("cpu.{n}", &inside, &locks)).is_err());
    std::fs::remove_file(&inside).unwrap();
    assert!(t0.elapsed() < Duration::from_secs(5), "a site file blocked");
    // a valid configuration without the CPU's lock file: never granted, nothing created
    let mut p = SitePolicy::new(cfg("cpu.{n}", &ledger, &locks)).unwrap();
    let a = p.acquire(1, "int5").unwrap();
    assert!(a.granted().is_none(), "granted without a lock file");
    assert!(
        !locks.join(format!("cpu.{cpu}")).exists(),
        "lock file created"
    );
    assert!(std::fs::read_dir(&locks).unwrap().next().is_none());
    // with the lock file: granted; the ledger grew by appends only
    std::fs::write(locks.join(format!("cpu.{cpu}")), b"").unwrap();
    let lease = p.acquire(1, "int5").unwrap().granted().expect("granted");
    drop(lease);
}

/// gate (c) on the int5 code (found by the gate run: at RLIMIT_NOFILE 14 the supervisor read OUTCOME at settle with
/// EMFILE, committed an accepting finish without the OUTCOME file, and the ledger's refusal stopped it with exit 70):
/// for every open-file limit 12..=20 the run ends with 0 or 71 (the supervisor's own resources), NEVER 70 and never a
/// verdict from the supervisor's lack of descriptors; an exit 0 has the reference result; the resumed run equals it.
#[test]
fn open_file_limits_never_end_in_an_internal_error_or_another_result() {
    let _g = serial();
    let t = Tmp::new("g3-nofile");
    let syn =
        serde_json::json!({"items": 12, "nodes": 2, "rungs": 2, "case_ms": [300, 1500], "seed": 9});
    let spec_p = write_spec(
        &t,
        "spec.json",
        &spec(&t, syn, serde_json::json!({"max_jobs": 4})),
    );
    let r = t.cdir("reference");
    // procfix5 (gate4 m3/m4): a synthetic item that crashed twice ends in attention: the reference may finish with 10
    let ref_codes = run_to_end(&spec_p, &r, &[]);
    assert!(
        ref_codes.len() == 1 && finished(ref_codes[0]),
        "{ref_codes:?}"
    );
    let ref_code = ref_codes[0];
    let reference = result(&r);
    let mut summary = Vec::new();
    for limit in 12u64..=20 {
        let d = t.cdir(&format!("nofile{limit}"));
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(d.with_extension("log"))
            .unwrap();
        let mut cmd = Command::new(SUP);
        cmd.args(["run", "--plain", "--spec"])
            .arg(&spec_p)
            .arg("--dir")
            .arg(&d)
            .stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .env_remove("RR_CAMPAIGN_FAILPOINT");
        // SAFETY: setrlimit is async-signal-safe; no allocation in the closure.
        unsafe {
            cmd.pre_exec(move || {
                let lim = libc::rlimit {
                    rlim_cur: limit,
                    rlim_max: limit,
                };
                libc::setrlimit(libc::RLIMIT_NOFILE, &lim);
                Ok(())
            });
        }
        let mut c = cmd.spawn().unwrap();
        let code = wait(&mut c, Duration::from_secs(300));
        summary.push(format!("nofile {limit}: exit {code}"));
        assert!(
            code == ref_code || code == 71,
            "nofile {limit}: exit {code} (only {ref_code} or 71): {}",
            log_tail(&d)
        );
        if code == ref_code {
            assert_eq!(
                result(&d),
                reference,
                "nofile {limit}: exit {code} with another result"
            );
        }
        if d.join("ledger").join("MANIFEST.json").exists() {
            let codes = run_to_end(&spec_p, &d, &[]);
            assert_eq!(
                codes.last(),
                Some(&ref_code),
                "nofile {limit} resume: {codes:?}"
            );
            assert_eq!(result(&d), reference, "nofile {limit}: resumed result");
        }
    }
    eprintln!("{}", summary.join("\n"));
    assert_no_marked(&t.campaign);
}
