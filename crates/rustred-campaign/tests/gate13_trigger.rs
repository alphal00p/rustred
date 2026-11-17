//! gate13 audit (WS/notes/implementer.gate13.md), item 1: the round-8b kill trigger of ac/procfix7 (df10b438,
//! supervisor/mod.rs poll_children: a job still alive 10 s after its launch without a verified scope is SIGKILLed through
//! `kill_now` = its PINNED group and the individual targets of the latest snapshot) must never signal a process that is
//! not the campaign's own, also when pids are REUSED by decoys at the worst moment.
//!
//! Setup (test-build failpoint `scope_pending:1`: every containment stays `Pending`, so EVERY attempt is ended by the
//! trigger at 10 s and the third one of a job stops the run, exit 71). The job program is a wrapper that
//! - records the leader pid (`L <pid>`),
//! - starts a HELPER that stays in the job's SESSION but leads its own process group (setpgid), carries the job's marker
//!   (inherited), is double-forked (re-parented to the supervisor, a child subreaper, which reaps it), and exits
//!   9.70 + 0.05 k s after it started (k = wrappers started so far, mod 8): just before or just after the trigger,
//! - execs the fake child (60 s).
//!
//! A SQUATTER thread of the test (pid 1 of the private namespace) waits until each recorded leader / helper pid is
//! released (reaped) and at once forks a DECOY at that very pid (`/proc/sys/kernel/ns_last_pid` = pid - 1, writable by
//! the namespace's root): `setsid sleep 600`, so a squatted leader pid is also the decoy's session and group number (the
//! old pinned sid). Half of the decoys carry a marker of ANOTHER campaign key with the same campaign name. With a slow
//! sampler (`sampler_ms` 7000) the trigger's snapshot is usually older than the helper's death: the stale snapshot then
//! names the decoy's pid as a member of the pinned session (the `PinnedSession` individual target).
//! Plain decoys (own session; one with the foreign marker) run from before the start.
//!
//! PASS: exit 71 after >= 3 `scope_unverified_ended`; EVERY decoy alive at the end, not stopped, same start time (a
//! group or process signal of any lethal kind would have ended it; SIGSTOP would show as T); no `signal_sent` process
//! record names a decoy (pid and start time).
//!
//! Spawns processes and sends signals: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::Event;
use std::collections::BTreeSet;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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

fn which(name: &str) -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|p| {
            std::env::split_paths(&p)
                .map(|d| d.join(name))
                .find(|f| f.is_file())
        })
        .unwrap_or_else(|| panic!("{name} not on PATH"))
}

fn wrapper(dir: &Path) -> PathBuf {
    let p = dir.join("wrap.sh");
    let py = which("python3");
    let helper = "import os,sys,time\nos.setpgid(0,0)\nd,n=sys.argv[1],int(sys.argv[2])\n\
                  f=open(d+'/pids.txt','a'); f.write('H %d\\n' % os.getpid()); f.close()\n\
                  time.sleep(9.70+0.05*(n%8))\n";
    let script = format!(
        "#!/bin/sh\nD={d}\nn=$(cat \"$D/pids.txt\" 2>/dev/null | grep -c '^L ')\necho \"L $$\" >> \"$D/pids.txt\"\n\
         ( {py} -I -c \"{helper}\" \"$D\" \"$n\" & )\nexec {FAKE} \"$@\"\n",
        d = dir.display(),
        py = py.display(),
        helper = helper.replace('"', "\\\""),
    );
    std::fs::write(&p, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// A decoy of the test: `setsid sleep 600`, optionally with one environment pair.
fn spawn_decoy(cwd: &Path, env: Option<(&str, &str)>) -> Child {
    let mut c = Command::new(sleep_bin());
    c.arg("600")
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some((k, v)) = env {
        c.env(k, v);
    }
    // SAFETY: setsid is async-signal-safe.
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    c.spawn().unwrap()
}

fn state_of(pid: u32) -> Option<char> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    s[s.rfind(')')? + 2..].chars().next()
}

struct Decoy {
    kind: String,
    /// The pid the squatter aimed at (0: a plain decoy).
    aimed: u32,
    child: Child,
    pid: u32,
    start: u64,
    at: Instant,
}

#[test]
fn g13_trigger_never_signals_a_decoy_at_a_reused_pid() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("g13-trig");
    let foreign = format!(
        "{}.ffffffffffffffff:N0.i0001.L0-t0:a1",
        t.campaign
    );
    let decoys: Arc<Mutex<Vec<Decoy>>> = Arc::new(Mutex::new(Vec::new()));
    let t0 = Instant::now();
    for (i, env) in [
        None,
        Some(("RR_CAMPAIGN_JOB", foreign.as_str())),
        None,
    ]
    .into_iter()
    .enumerate()
    {
        let c = spawn_decoy(&t.path, env);
        let pid = c.id();
        decoys.lock().unwrap().push(Decoy {
            kind: format!("plain{i}{}", if env.is_some() { "+foreign-marker" } else { "" }),
            aimed: 0,
            pid,
            start: start_ticks(pid),
            child: c,
            at: t0,
        });
    }
    let wrap = wrapper(&t.path);
    let mut v = spec(
        &t,
        serde_json::json!({"items": 6, "nodes": 1, "rungs": 1, "seed": 5, "case_ms": [60000, 60000],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 3, "sampler_ms": 7000}),
    );
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    let sp = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");

    // the squatter
    let stop = Arc::new(AtomicBool::new(false));
    let sq = {
        let stop = Arc::clone(&stop);
        let decoys = Arc::clone(&decoys);
        let dir = t.path.clone();
        let foreign = foreign.clone();
        std::thread::spawn(move || {
            let mut seen: BTreeSet<(char, u32)> = BTreeSet::new();
            let mut watch: Vec<(char, u32)> = Vec::new();
            let mut n = 0usize;
            while !stop.load(Ordering::SeqCst) {
                if let Ok(s) = std::fs::read_to_string(dir.join("pids.txt")) {
                    for l in s.lines() {
                        let mut w = l.split_whitespace();
                        if let (Some(k), Some(p)) = (w.next(), w.next())
                            && let (Some(k), Ok(p)) = (k.chars().next(), p.parse::<u32>())
                            && seen.insert((k, p))
                        {
                            watch.push((k, p));
                        }
                    }
                }
                let mut i = 0;
                while i < watch.len() {
                    let (k, p) = watch[i];
                    if Path::new(&format!("/proc/{p}")).exists() {
                        i += 1;
                        continue;
                    }
                    watch.swap_remove(i);
                    let _ = std::fs::write("/proc/sys/kernel/ns_last_pid", format!("{}", p - 1));
                    n += 1;
                    let env = (n % 2 == 0).then_some(("RR_CAMPAIGN_JOB", foreign.as_str()));
                    let c = spawn_decoy(&dir, env);
                    let pid = c.id();
                    let start = start_ticks(pid);
                    decoys.lock().unwrap().push(Decoy {
                        kind: format!(
                            "squat-{}{}",
                            if k == 'L' { "leader" } else { "helper" },
                            if env.is_some() { "+foreign-marker" } else { "" }
                        ),
                        aimed: p,
                        child: c,
                        pid,
                        start,
                        at: Instant::now(),
                    });
                }
                std::thread::sleep(Duration::from_micros(500));
            }
        })
    };

    let mut c = start(
        &sp,
        &d,
        &[],
        &[("RR_CAMPAIGN_FAILPOINT", "scope_pending:1")],
    );
    let code = wait(&mut c, Duration::from_secs(600));
    // the end-of-job hunts and the final sweep are over; keep the squatter a little longer for the last reaps
    std::thread::sleep(Duration::from_secs(2));
    stop.store(true, Ordering::SeqCst);
    sq.join().unwrap();

    let recs = records(&d);
    let words: Vec<String> = notes(&d, "job_containment ")
        .iter()
        .map(|n| {
            let j: serde_json::Value =
                serde_json::from_str(n.trim_start_matches("job_containment ")).unwrap();
            j["containment"].as_str().unwrap_or("?").to_string()
        })
        .collect();
    let mut ds = decoys.lock().unwrap();
    let mut bad = Vec::new();
    let (mut hit_l, mut hit_h, mut miss) = (0, 0, 0);
    for x in ds.iter_mut() {
        let alive = x.child.try_wait().unwrap().is_none();
        let st = state_of(x.pid);
        let same = alive && start_ticks(x.pid) == x.start;
        if x.aimed != 0 {
            if x.pid == x.aimed {
                if x.kind.contains("leader") {
                    hit_l += 1
                } else {
                    hit_h += 1
                }
            } else {
                miss += 1
            }
        }
        if !alive || !same || st == Some('T') || st == Some('t') {
            bad.push(format!(
                "decoy {} pid {} (aimed {}) alive {alive} state {st:?} same-start {same}",
                x.kind, x.pid, x.aimed
            ));
        }
    }
    let mut sigs = 0;
    let mut by_proof = std::collections::BTreeMap::<String, usize>::new();
    for r in &recs {
        if let Event::SignalSent {
            pid,
            start_ticks,
            target,
            proof,
            signal,
            ..
        } = &r.event
        {
            sigs += 1;
            *by_proof.entry(format!("{proof}/{target}/{signal}")).or_default() += 1;
            for x in ds.iter() {
                // a process record names (pid, start time): exact. A GROUP record (no start time) names the pinned
                // leader's number, which a squatted leader decoy reuses AFTER the legitimate group signals of the
                // end-of-job hunt (records are committed later, so their time does not order them against the squat):
                // a group signal that reached a decoy is caught by the decoy's liveness / state check above instead.
                let named = matches!(start_ticks, Some(s) if *pid == x.pid && *s == x.start);
                if named {
                    bad.push(format!(
                        "signal_sent {signal} {target} pid {pid} start {start_ticks:?} proof {proof:?} names decoy {} (seq {})",
                        x.kind, r.seq
                    ));
                }
            }
        }
    }
    let summary = format!(
        "G13 trigger: exit {code}; containment {:?}; decoys {} (plain {}, squatted leader {hit_l}, squatted helper {hit_h}, \
         squat missed the pid {miss}); signal_sent {sigs} {by_proof:?}; t {:?}",
        words.iter().fold(std::collections::BTreeMap::<&str, usize>::new(), |mut m, w| {
            *m.entry(w.as_str()).or_default() += 1;
            m
        }),
        ds.len(),
        ds.iter().filter(|x| x.aimed == 0).count(),
        t0.elapsed()
    );
    eprintln!("{summary}");
    std::fs::write(t.path.join("g13_summary.txt"), &summary).ok();
    for x in ds.iter() {
        eprintln!(
            "G13 decoy {} pid {} aimed {} start {} after {:?}",
            x.kind,
            x.pid,
            x.aimed,
            x.start,
            x.at.duration_since(t0)
        );
    }
    // clean up OUR decoys (own children, unreaped: their pids are pinned)
    for x in ds.iter_mut() {
        let _ = x.child.kill();
        let _ = x.child.wait();
    }
    kill_marked(&t.campaign);
    assert!(bad.is_empty(), "{bad:#?}\n{summary}");
    assert_eq!(code, 71, "{summary}\n{}", log_tail(&d));
    assert!(!words.iter().any(|w| w == "marker_only"), "{summary}");
    assert!(
        words.iter().filter(|w| *w == "scope_unverified_ended").count() >= 3,
        "{summary}"
    );
    assert!(
        hit_l + hit_h >= 4,
        "too few squats at the aimed pid to mean anything: {summary}"
    );
}
