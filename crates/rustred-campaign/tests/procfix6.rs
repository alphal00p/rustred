//! Lane procfix6 (the fifth gate audit's MINOR findings, WS/notes/implementer.gate5.md). Spawns processes and sends
//! signals: run ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10); every test refuses
//! to run outside one.
//! - m1: a sweep never leaves a process it STOPPED stopped (flipping members that drop the marker between the batch's
//!   read and the stop); decoys (unmarked, another campaign's flipping marker) are never touched;
//! - m2: the spawn barrier's pipe cannot be reached through /proc/<pid>/fd by a process of the same uid without
//!   CAP_SYS_PTRACE, and even a writer that reaches it cannot release it without the token; the supervisor that holds
//!   the pipes is non-dumpable;
//! - m3: a full intent log ends a stop with exit 75 promptly, naming what was not signalled, never "pinned groups
//!   killed";
//! - m4: a child killed by SIGXFSZ is a resource end (row 15b), retried after the back-off, never a crash;
//! - m5: one ledger note per job names the containment in force; no scope unit is recorded where no scope exists.

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use rustred_campaign::resources::{MARKER_VAR, read_stat};
use rustred_campaign::state::FinishClass;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
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

fn state_of(pid: u32) -> Option<char> {
    read_stat(pid).ok().flatten().map(|s| s.state)
}

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// Empty the capability bounding set before the exec: the program then runs as uid 0 of the namespace WITHOUT any
/// capability, like a process of the user on the host. (Inside the namespace every uid-0 process holds all capabilities,
/// and the kernel's capability-subset rule of `cap_ptrace_access_check` alone would hide such a process from a
/// capability-less reader, dumpable or not: both the reader AND the targets must be capability-less for a meaningful
/// check of PR_SET_DUMPABLE.)
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

fn finish(mut c: Child) {
    let _ = c.kill();
    let _ = c.wait();
}

fn notes(d: &Path, prefix: &str) -> Vec<String> {
    records(d)
        .into_iter()
        .filter_map(|r| match r.event {
            Event::Note { text } if text.starts_with(prefix) => Some(text),
            _ => None,
        })
        .collect()
}

/// gate5 m1: members that flip between a marked and an unmarked image (exec in a loop, same pid) race the sweep's
/// batch (environ reads, ONE intent sync, then the SIGSTOPs). Whatever the race does, after the sweep no member is
/// STOPPED: each is dead (killed through the marker, or by the identity held at its stop), or running (continued and
/// reported, or never caught: the documented marker-dropping gap). Nothing is stranded. Decoys (an unmarked sleeper and
/// a member of ANOTHER campaign flipping the same way) are alive, not stopped, never in the report.
#[test]
fn m1_flipping_members_are_never_left_stopped_and_decoys_untouched() {
    if !in_ns() {
        return;
    }
    use rustred_campaign::launch::sweep::{SweepSpec, sweep};
    let _g = serial();
    let t = Tmp::new("pf6-m1");
    let flip = script(
        &t.path,
        "flip.sh",
        "#!/bin/sh\nif [ -n \"$RR_CAMPAIGN_JOB\" ]; then exec env -u RR_CAMPAIGN_JOB \"$0\"; \
         else exec env RR_CAMPAIGN_JOB=\"$FLIPMARK\" \"$0\"; fi\n",
    );
    let id = "pf6flip.0123456789abcdef";
    let other = "pf6othr.0123456789abcdef:job-t0:a1";
    let bound = read_stat(std::process::id()).unwrap().unwrap().start_ticks;
    let spawn_flip = |mark: &str| {
        Command::new(&flip)
            .env_clear()
            .env("PATH", path_env())
            .env("FLIPMARK", mark)
            .env(MARKER_VAR, mark)
            .stdin(Stdio::null())
            .spawn()
            .unwrap()
    };
    let (mut held, mut continued, mut evaded) = (0usize, 0usize, 0usize);
    for round in 0..5 {
        let mut members: Vec<Child> = (0..12)
            .map(|_| spawn_flip(&format!("{id}:job-t0:a1")))
            .collect();
        let decoys = vec![
            spawn_flip(other),
            Command::new(sleep_bin())
                .arg("600")
                .env_clear()
                .spawn()
                .unwrap(),
        ];
        std::thread::sleep(Duration::from_millis(200));
        let r = sweep(
            &SweepSpec {
                campaign_id: id.into(),
                only_attempt: None,
                not_before_ticks: bound,
                budget: Duration::from_secs(20),
                pass_gap: Duration::from_millis(20),
                narrowing: None,
            },
            &std::sync::atomic::AtomicBool::new(false),
        );
        held += r.killed_by_hold.len();
        continued += r.continued.len();
        assert_eq!(r.stranded().count(), 0, "round {round}: {r:?}");
        std::thread::sleep(Duration::from_millis(100));
        for m in &mut members {
            if m.try_wait().unwrap().is_some() {
                continue;
            }
            let pid = m.id();
            assert_ne!(
                state_of(pid),
                Some('T'),
                "round {round}: member {pid} left STOPPED; report {r:?}"
            );
            if !r.continued.iter().any(|c| c.process.pid == pid) {
                evaded += 1;
            }
        }
        for d in &decoys {
            let pid = d.id();
            assert_ne!(state_of(pid), Some('T'), "decoy {pid} stopped");
            assert!(state_of(pid).is_some(), "decoy {pid} gone");
            assert!(
                !r.killed.iter().any(|k| k.pid == pid)
                    && !r.continued.iter().any(|k| k.process.pid == pid)
                    && !r.left.iter().any(|k| k.process.pid == pid),
                "decoy {pid} named in the report {r:?}"
            );
        }
        for m in members.into_iter().chain(decoys) {
            finish(m);
        }
    }
    eprintln!(
        "m1 flippers: {held} killed by the stop hold, {continued} continued, {evaded} never caught (5 rounds x 12)"
    );
}

/// A barrier shim of the real binary waiting on a pipe: (shim, our write end, the inherited read fd number).
fn barrier_shim(token: &[u8; 32], ran: &Path) -> (Child, OwnedFd, i32) {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: pipe2 fills two descriptors.
    assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
    // SAFETY: fresh descriptors owned by nobody else.
    let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    let rfd = r.as_raw_fd();
    let mut c = Command::new(SUP);
    c.arg("__spawn-barrier")
        .arg(rfd.to_string())
        .arg(blake3::hash(token).to_hex().to_string())
        .arg("--")
        .arg("sh")
        .arg("-c")
        .arg(format!("echo RAN > {}", ran.display()))
        .env_clear()
        .env("PATH", path_env())
        .stdin(Stdio::null());
    use std::os::unix::process::CommandExt;
    // SAFETY: fcntl is async-signal-safe; it clears close-on-exec of the read end in the child only.
    unsafe {
        c.pre_exec(move || {
            libc::fcntl(rfd, libc::F_SETFD, 0);
            Ok(())
        });
    }
    drop_caps(&mut c);
    let child = c.spawn().unwrap();
    drop(r);
    // wait for its exec into the shim, then a moment for its prctl
    let t0 = Instant::now();
    while !std::fs::read(format!("/proc/{}/cmdline", child.id()))
        .is_ok_and(|b| String::from_utf8_lossy(&b).contains("__spawn-barrier"))
    {
        assert!(t0.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(100));
    (child, w, rfd)
}

/// A same-uid process WITHOUT capabilities (bounding set emptied before its exec as uid 0 of the namespace) that tries
/// for 1.5 s to write into `/proc/<pid>/fd/<fd>`; returns (successful writes, attempts, its CapEff line).
fn capless_writer(t: &Tmp, pid: u32, fd: i32, tag: &str) -> (u64, u64, String) {
    let out = t.path.join(format!("writer-{tag}.txt"));
    let body = format!(
        "#!/bin/sh\ncap=$(grep CapEff /proc/self/status)\nok=0; n=0; end=$(( $(date +%s) + 2 ))\n\
         while [ $(date +%s) -lt $end ]; do\n  if ( printf G > /proc/{pid}/fd/{fd} ) 2>/dev/null; then ok=$((ok+1)); fi\n  \
         n=$((n+1))\ndone\necho \"$ok $n $cap\" > {out}\n",
        out = out.display()
    );
    let s = script(&t.path, &format!("writer-{tag}.sh"), &body);
    let mut c = Command::new(&s);
    c.env_clear().env("PATH", path_env()).stdin(Stdio::null());
    drop_caps(&mut c);
    let st = c.spawn().unwrap().wait().unwrap();
    assert!(st.success());
    let txt = std::fs::read_to_string(&out).unwrap();
    let mut it = txt.split_whitespace();
    let ok = it.next().unwrap().parse().unwrap();
    let n = it.next().unwrap().parse().unwrap();
    (ok, n, txt)
}

/// gate5 m2: the barrier bypass of the audit (`barrier_inner.sh C`: a same-uid process writes into
/// /proc/<shim>/fd/<fd>) is closed twice. (1) The shim is non-dumpable: a capability-less writer of the same uid
/// reaches the pipe of a DUMPABLE capability-less process (control) but not the (capability-less) shim's. (2) A writer that does reach it (here: the test
/// itself, uid 0 with CAP_SYS_PTRACE in the namespace) cannot release it: a wrong token ends the shim with 125 and the
/// program never runs; EOF too; only the token releases it.
#[test]
fn m2_barrier_needs_the_release_token_and_its_pipe_is_unreachable() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf6-m2");
    let token: [u8; 32] = *blake3::hash(t.campaign.as_bytes()).as_bytes();
    // control: a dumpable capability-less process holding a pipe (its stdin) is reachable by the capability-less writer
    let mut ctl = {
        let mut c = Command::new(sleep_bin());
        c.arg("30").stdin(Stdio::piped());
        drop_caps(&mut c);
        c.spawn().unwrap()
    };
    let (ok, n, txt) = capless_writer(&t, ctl.id(), 0, "ctl");
    assert!(txt.contains("CapEff:\t0000000000000000"), "{txt}");
    assert!(
        ok > 0 && n > 0,
        "control: the writer reaches a dumpable process's pipe ({txt})"
    );
    let _ = ctl.kill();
    let _ = ctl.wait();
    // (1) the shim's pipe is not reachable; the program does not run; the shim keeps waiting
    let ran = t.path.join("ran1.txt");
    let (mut shim, w, fd) = barrier_shim(&token, &ran);
    let (ok, n, txt) = capless_writer(&t, shim.id(), fd, "shim");
    assert_eq!(ok, 0, "the shim's pipe was reached ({n} tries): {txt}");
    assert!(shim.try_wait().unwrap().is_none(), "the shim keeps waiting");
    assert!(!ran.exists());
    // (2) a writer that reaches it (CAP_SYS_PTRACE in the namespace) without the token: 125, never ran
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .open(format!("/proc/{}/fd/{fd}", shim.id()))
            .expect("the test holds CAP_SYS_PTRACE in its namespace");
        f.write_all(b"GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG").unwrap();
    }
    let st = shim.wait().unwrap();
    assert_eq!(st.code(), Some(125), "wrong token");
    assert!(!ran.exists(), "the program ran without the token");
    drop(w);
    // EOF: 125, never ran
    let ran = t.path.join("ran2.txt");
    let (mut shim, w, _) = barrier_shim(&token, &ran);
    drop(w);
    assert_eq!(shim.wait().unwrap().code(), Some(125));
    assert!(!ran.exists());
    // the token releases it
    let ran = t.path.join("ran3.txt");
    let (mut shim, w, _) = barrier_shim(&token, &ran);
    // SAFETY: a 32-byte buffer to our own pipe.
    let r = unsafe { libc::write(w.as_raw_fd(), token.as_ptr().cast(), 32) };
    assert_eq!(r, 32);
    drop(w);
    assert_eq!(shim.wait().unwrap().code(), Some(0));
    assert_eq!(std::fs::read_to_string(&ran).unwrap().trim(), "RAN");
}

/// gate5 m2: the supervisor (the real binary, which hosts the barrier and holds its pipes) is non-dumpable: a
/// capability-less process of the same uid cannot read its environ (nor its descriptors), while a job's program
/// (dumpable again after its exec) stays readable, so the marker scans keep working; the campaign ends normally. The
/// supervisor runs capability-less here (as on the host), so it also shows that it needs no capability in a namespace.
#[test]
fn m2_supervisor_is_not_dumpable_and_jobs_are() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf6-m2s");
    let syn = serde_json::json!({"items": 1, "case_ms": [6000, 6000], "seed": 5, "p_mem": 0.0, "p_crash": 0.0, "p_norule": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let d = t.cdir("c");
    let mut c = {
        let log = std::fs::File::create(d.with_extension("log")).unwrap();
        let mut c = Command::new(SUP);
        c.arg("run")
            .arg("--spec")
            .arg(&spec_p)
            .arg("--dir")
            .arg(&d)
            .stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .env_remove("RR_CAMPAIGN_FAILPOINT");
        drop_caps(&mut c);
        c.spawn().unwrap()
    };
    let t0 = Instant::now();
    let job_pid = loop {
        if !d.join("ledger").join("MANIFEST.json").exists() {
            assert!(t0.elapsed() < Duration::from_secs(120), "{}", log_tail(&d));
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        let started = records(&d).into_iter().find_map(|r| match r.event {
            Event::Job {
                ev: JobEvent::Started { pid, .. },
                ..
            } => Some(pid),
            _ => None,
        });
        if let Some(p) = started {
            break p;
        }
        assert!(t0.elapsed() < Duration::from_secs(120), "{}", log_tail(&d));
        std::thread::sleep(Duration::from_millis(100));
    };
    std::thread::sleep(Duration::from_millis(500));
    let out = t.path.join("reads.txt");
    let s = script(
        &t.path,
        "reader.sh",
        &format!(
            "#!/bin/sh\nfor p in {sup} {job}; do if cat /proc/$p/environ > /dev/null 2>&1; then echo \"$p readable\"; \
             else echo \"$p unreadable\"; fi; if readlink /proc/$p/fd/0 > /dev/null 2>&1; then echo \"$p fd-readable\"; \
             else echo \"$p fd-unreadable\"; fi; done > {out}\n",
            sup = c.id(),
            job = job_pid,
            out = out.display()
        ),
    );
    let mut cmd = Command::new(&s);
    cmd.env_clear().env("PATH", path_env());
    drop_caps(&mut cmd);
    assert!(cmd.status().unwrap().success());
    let reads = std::fs::read_to_string(&out).unwrap();
    assert!(reads.contains(&format!("{} unreadable", c.id())), "{reads}");
    assert!(
        reads.contains(&format!("{} fd-unreadable", c.id())),
        "{reads}"
    );
    assert!(reads.contains(&format!("{job_pid} readable")), "{reads}");
    assert_eq!(
        wait(&mut c, Duration::from_secs(300)),
        0,
        "{}",
        log_tail(&d)
    );
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

/// A contract program that, for `job`, starts a daemon carrying the job's marker in its own session (only a signal ends
/// it), records `pid start`, then runs the fake child.
fn daemon_wrapper(dir: &Path) -> PathBuf {
    script(
        dir,
        "wrap.sh",
        &format!(
            "#!/bin/sh\nif [ \"$1\" = job ]; then\n  setsid {sleep} 600 </dev/null >/dev/null 2>&1 &\n  d=$!\n  \
             echo \"$d $(cut -d' ' -f22 /proc/$d/stat)\" >> {pids}\nfi\nexec {fake} \"$@\"\n",
            sleep = sleep_bin().display(),
            pids = dir.join("daemons.txt").display(),
            fake = FAKE
        ),
    )
}

fn daemons(dir: &Path) -> Vec<(u32, u64)> {
    std::fs::read_to_string(dir.join("daemons.txt"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
        })
        .collect()
}

fn alive_with(pid: u32, start: u64) -> bool {
    read_stat(pid)
        .ok()
        .flatten()
        .is_some_and(|s| s.start_ticks == start && !s.is_dead())
}

/// gate5 m3: when no signal intent can be made durable (test build: `intent_enospc:1`, every intent batch fails as on a
/// full disk), a stop ends with exit 75 promptly (the ledger-dead path: the grace, then exit), never with the stop code
/// after the hunt bound; the log names every signal NOT sent (pid, label) and the exit line counts real deliveries
/// (never "(pinned groups killed)"); nothing was sent (the daemons live); the same command with space recovers.
#[test]
fn m3_full_intent_log_ends_75_promptly_and_names_what_was_not_signalled() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf6-m3");
    let wrap = daemon_wrapper(&t.path);
    let syn = serde_json::json!({"items": 1, "nodes": 1, "rungs": 1, "deps": 0.0, "case_ms": [600000, 600000], "seed": 9, "p_mem": 0.0, "p_crash": 0.0, "p_norule": 0.0});
    let mut v = spec(&t, syn, serde_json::json!({"grace_seconds": 2}));
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    let spec_p = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let mut c = start(
        &spec_p,
        &d,
        &[],
        &[("RR_CAMPAIGN_FAILPOINT", "intent_enospc:1")],
    );
    let t0 = Instant::now();
    while daemons(&t.path).is_empty() {
        assert!(t0.elapsed() < Duration::from_secs(120), "{}", log_tail(&d));
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_secs(1));
    // SAFETY: test code; our own direct child (the supervisor), not reaped.
    unsafe { libc::kill(c.id() as i32, libc::SIGINT) };
    let t1 = Instant::now();
    let code = wait(&mut c, Duration::from_secs(150));
    let took = t1.elapsed();
    let log = std::fs::read_to_string(d.join("log/supervisor.log")).unwrap_or_default();
    assert_eq!(code, 75, "{log}");
    assert!(took < Duration::from_secs(45), "stop took {took:?}: {log}");
    assert!(log.contains("NOT sent to"), "{log}");
    assert!(
        log.contains("signal intent log cannot be written (disk full)"),
        "{log}"
    );
    assert!(
        log.contains("signal(s) NOT sent because their intent could not be written"),
        "{log}"
    );
    assert!(!log.contains("(pinned groups killed)"), "{log}");
    for (p, s) in daemons(&t.path) {
        assert!(alive_with(p, s), "daemon {p}: nothing may have been sent");
    }
    // the same command without the failpoint: recovery stops what is left
    let mut c = start(&spec_p, &d, &[], &[]);
    let t2 = Instant::now();
    while records(&d)
        .iter()
        .filter(|r| {
            matches!(
                r.event,
                Event::Job {
                    ev: JobEvent::Started { .. },
                    ..
                }
            )
        })
        .count()
        < 2
    {
        assert!(t2.elapsed() < Duration::from_secs(120), "{}", log_tail(&d));
        std::thread::sleep(Duration::from_millis(100));
    }
    for (p, s) in daemons(&t.path).into_iter().take(1) {
        let t3 = Instant::now();
        while alive_with(p, s) {
            assert!(
                t3.elapsed() < Duration::from_secs(30),
                "daemon {p} survived the recovery"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    // SAFETY: as above.
    unsafe { libc::kill(c.id() as i32, libc::SIGINT) };
    assert_eq!(
        wait(&mut c, Duration::from_secs(150)),
        130,
        "{}",
        log_tail(&d)
    );
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

/// gate5 m4, as amended by procfix7 (gate6 m4-a): a child killed by SIGXFSZ is a RESOURCE end only with the
/// supervisor's OWN evidence (its finite file-size limit, or a full file system; lib test
/// `m4_disk_attributed_child_ends_are_resource_ends`). Here the first try of every item (job `...-t0`, attempt 1)
/// lowers ITS OWN limit (`ulimit -f 0` in the job's wrapper) on a roomy disk under an unlimited supervisor: that is the
/// child's doing, a CRASH (one per item, below the crashed-twice rule; the synthetic source retries a crash as the
/// item's next job `...-t1`): no row-15b requeue, every item accepted on its retry, exit 0.
#[test]
fn m4_sigxfsz_death_is_a_resource_end_not_a_crash() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf6-m4");
    let syn = serde_json::json!({"items": 3, "case_ms": [200, 400], "seed": 4, "p_mem": 0.0, "p_crash": 0.0, "p_norule": 0.0});
    let mut v = spec(&t, syn, serde_json::json!({}));
    let r = t.cdir("reference");
    let spec_ref = write_spec(&t, "ref.json", &v);
    assert_eq!(run_to_end(&spec_ref, &r, &[]), vec![0]);
    let wrap = script(
        &t.path,
        "wrap.sh",
        &format!(
            "#!/bin/sh\nif [ \"$1\" = job ]; then\n  case \"$2\" in *-t0.a1/job.json) ulimit -f 0;; esac\nfi\n\
             exec {FAKE} \"$@\"\n"
        ),
    );
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    let spec_p = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let mut c = start(&spec_p, &d, &[], &[]);
    assert_eq!(
        wait(&mut c, Duration::from_secs(600)),
        0,
        "{}",
        log_tail(&d)
    );
    let crashed = count(&d, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Finished {
                    class: FinishClass::Crashed,
                    ..
                },
                ..
            }
        )
    });
    assert_eq!(crashed, 3, "one crash per job: {}", log_tail(&d));
    let resource = count(
        &d,
        |e| matches!(e, Event::Job { ev: JobEvent::Requeue { reason }, .. } if reason.ends_with("(row 15b)")),
    );
    assert_eq!(resource, 0, "{}", log_tail(&d));
    let log = std::fs::read_to_string(d.join("log/supervisor.log")).unwrap_or_default();
    assert!(!log.contains("a resource end (row 15b)"), "{log}");
    assert_eq!(result(&d).accepted.len(), result(&r).accepted.len());
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

/// gate5 m5: one ledger note `job_containment` per started attempt names the containment in force (inside a pid
/// namespace there is no systemd scope: `marker_only`), the start log names the mechanism, and no launch record names
/// a scope unit (none exists).
#[test]
fn m5_containment_is_named_per_job_and_no_phantom_scope_unit() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf6-m5");
    let syn = serde_json::json!({"items": 4, "case_ms": [200, 400], "seed": 2, "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let d = t.cdir("c");
    assert_eq!(run_to_end(&spec_p, &d, &[]), vec![0]);
    let started = count(&d, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Started { .. },
                ..
            }
        )
    });
    let launches_with_scope = count(&d, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Launch {
                    scope_unit: Some(_),
                    ..
                },
                ..
            }
        )
    });
    let n = notes(&d, "job_containment ");
    assert!(started > 0);
    assert_eq!(n.len(), started, "{n:#?}");
    assert!(
        n.iter()
            .all(|x| x.contains("\"containment\":\"marker_only\"")),
        "{n:#?}"
    );
    assert_eq!(launches_with_scope, 0);
    let log = std::fs::read_to_string(d.join("log/supervisor.log")).unwrap_or_default();
    assert!(log.contains("containment: marker only"), "{log}");
    assert_no_marked(&t.campaign);
}
