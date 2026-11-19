//! Lane procfix7 round 7 (gate 11, WS/notes/implementer.gate11.md): G11-M1 (the completeness walk over every thread),
//! M11-2 (the CPU policy wiring: the host step's site.json must launch jobs), and the round-6 mutations no test caught
//! (m11-mut: M4 a pid reused between the scan and the stop, M9 a supervisor that is not a child subreaper; M6 and M8 have
//! their own tests: tests/procfix11_seed.rs, the cgroup unit test of launch/cgroup.rs).
//!
//! Spawns processes and sends signals: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const WS: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev";

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

fn start_ticks(pid: u32) -> Option<u64> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    s[s.rfind(')')? + 2..]
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

/// Test code: SIGKILL one exact pid of ours (the shell's builtin).
fn kill_pid(pid: u32) {
    let _ = Command::new("sh")
        .args(["-c", &format!("kill -KILL {pid} 2>/dev/null")])
        .env("PATH", path_env())
        .status();
}

/// Test code as the namespace's init (pid 1): reap one exact pid re-parented to us, if it is a zombie now.
fn reap(pid: u32) {
    // SAFETY: waitpid on one exact pid with a null status pointer
    unsafe { libc::waitpid(pid as i32, std::ptr::null_mut(), libc::WNOHANG) };
}

/// Build one of the gate helpers (C) into `dir`; None without a C compiler.
fn cc(dir: &Path, rel_src: &str, out: &str, so: bool) -> Option<PathBuf> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts")
        .join(rel_src);
    let o = dir.join(out);
    let mut c = Command::new("cc");
    if so {
        c.args(["-shared", "-fPIC"]);
    }
    c.args(["-O1", "-o"]).arg(&o).arg(&src);
    c.args(if so { vec!["-ldl"] } else { vec!["-lpthread"] });
    c.env("PATH", path_env())
        .status()
        .is_ok_and(|s| s.success())
        .then_some(o)
}

/// A spec whose first job runs `launch` (sh) once, then the fake child.
fn spec_once(t: &Tmp, launch: &str, extra_res: serde_json::Value) -> PathBuf {
    let once = t.path.join("once.lock");
    let wrap = t.path.join("wrap.sh");
    std::fs::write(
        &wrap,
        format!(
            "#!/bin/sh\nif [ \"$1\" = job ] && mkdir {once} 2>/dev/null; then\n  {launch}\nfi\nexec {fake} \"$@\"\n",
            once = once.display(),
            fake = FAKE
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrap, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut res = serde_json::json!({"max_jobs": 2, "unproven_end_limit_s": 60});
    for (k, v) in extra_res.as_object().unwrap() {
        res[k] = v.clone();
    }
    let mut v = spec(
        t,
        serde_json::json!({"items": 4, "nodes": 1, "rungs": 1, "seed": 5, "case_ms": [1500, 2500],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        res,
    );
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    write_spec(t, "spec.json", &v)
}

/// G11-M1, the second walk-blind family: an UNMARKED setsid'd subreaper (gate11 g11thr, mode `zleader`) whose MAIN thread
/// exits while a worker thread lives; the marked sleep it fathered through an intermediate is re-parented to that LIVE
/// NON-LEADER thread. Session 2 runs with this pid missing from both /proc enumerations (test-build fault
/// `proc_hide_pid`, root listing only here: `proc_drop_root_pid`); the walk over EVERY thread reaches it: never exit 0
/// with it alive.
#[test]
fn g11_zleader_orphan_on_a_non_leader_thread_is_reached_by_the_walk() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf11-zl");
    let Some(thr) = cc(&t.path, "gate11/src/g11thr.c", "g11thr", false) else {
        eprintln!("SKIPPED: no C compiler");
        return;
    };
    let rec = t.path.join("helper.pids");
    let launch = format!(
        "env -i {} {rec}.w \"$RR_CAMPAIGN_JOB\" zleader & \
         for i in $(seq 1 50); do [ -s {rec}.w ] && break; sleep 0.1; done; mv {rec}.w {rec}",
        thr.display(),
        rec = rec.display()
    );
    let sp = spec_once(&t, &launch, serde_json::json!({}));
    let d = t.cdir("c");
    let mut s1 = start(
        &sp,
        &d,
        &["--plain"],
        &[("PATH", path_env().to_str().unwrap())],
    );
    wait_until("the helper", Duration::from_secs(300), || rec.exists());
    std::thread::sleep(Duration::from_millis(800));
    signal(&s1, libc::SIGKILL);
    let _ = s1.wait();
    let f = std::fs::read_to_string(&rec).unwrap();
    let mut it = f.split_whitespace().map(|w| w.parse::<u32>().unwrap_or(0));
    let (m, p) = (it.next().unwrap(), it.next().unwrap_or(0));
    assert!(alive(m), "precondition: the marked process lives");
    // the marked sleep is on a NON-leader thread's list of p (its leader exited: a zombie with a live thread)
    let leader =
        std::fs::read_to_string(format!("/proc/{p}/task/{p}/children")).unwrap_or_default();
    assert!(
        !leader.split_whitespace().any(|w| w == m.to_string()),
        "precondition: {m} is not on the leader's list of {p}"
    );
    let on_thread = std::fs::read_dir(format!("/proc/{p}/task"))
        .unwrap()
        .flatten()
        .any(|e| {
            std::fs::read_to_string(e.path().join("children"))
                .unwrap_or_default()
                .split_whitespace()
                .any(|w| w == m.to_string())
        });
    assert!(on_thread, "precondition: {m} is on a thread's list of {p}");
    let fp = format!("proc_drop_root_pid:{m}");
    let code = run_env(&sp, &d, &[("RR_CAMPAIGN_FAILPOINT", &fp)]);
    let a = alive(m);
    eprintln!("PF11 zleader: exit {code}, marked {m} (orphan on a worker thread of {p}) alive {a}");
    kill_pid(m);
    if p > 1 {
        kill_pid(p);
    }
    assert!(
        !(code == 0 && a),
        "exit 0 with the marked {m} alive (on a non-leader thread of {p}): {}",
        log_tail(&d)
    );
}

fn run_env(sp: &Path, d: &Path, env: &[(&str, &str)]) -> i32 {
    let mut c = start(sp, d, &["--plain"], env);
    wait(&mut c, Duration::from_secs(900))
}

/// The CPU lists of RUNBOOK 1.7's site.json exclude lists.
const RUNBOOK_EXCLUDE: &[&str] = &[
    "168-175",
    "288-319",
    "128-162,164-166",
    "192-215,258-260,262,271,335",
    "342,362,363,378",
];

fn parse_list(s: &str) -> BTreeSet<usize> {
    rustred_campaign::resources::parse_cpu_list(s).unwrap_or_default()
}

/// M11-2: a healthy fake campaign under RUNBOOK 1.7's OWN site.json (idle_min 0.85, respect_foreign_pinning true, the
/// exclude lists, the held files and pause flags of WS/ops read-only) launches jobs and ends 0. Only the lock directory
/// and the CPU ledger are private copies (a test never takes the host's CPU flocks nor writes the shared ledger). Idleness
/// is the host's (/proc/stat is not namespaced). Every claimed CPU lies outside the exclude lists and the held lists,
/// at most max_cores at once, each RELEASED.
#[test]
fn m11_2_runbook_site_json_launches_jobs_and_ends_0() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    for p in [
        format!("{WS}/ops/PAUSE"),
        format!("{WS}/plan/auto-campaign/implementer_ops/PAUSE"),
    ] {
        if Path::new(&p).exists() {
            eprintln!(
                "SKIPPED: pause flag {p} present (the site policy launches nothing while it exists)"
            );
            return;
        }
    }
    let t = Tmp::new("pf11-site17");
    let locks = t.path.join("locks");
    std::fs::create_dir(&locks).unwrap();
    for c in rustred_campaign::resources::allowed_cpus().unwrap() {
        std::fs::write(locks.join(format!("cpu.{c}")), "").unwrap();
    }
    let ledger = t.path.join("CPU_LEDGER.md");
    std::fs::write(&ledger, "").unwrap();
    let site = t.path.join("site.json");
    std::fs::write(
        &site,
        serde_json::json!({
            "schema": "rustred.campaign.site-policy.v1", "tag": "rr-campaign",
            "lock_dir": locks, "lock_name": "cpu.{n}", "ledger_file": ledger, "ledger_scan_lines": 1500,
            "candidates": null, "exclude": RUNBOOK_EXCLUDE,
            "held_files": [
                {"path": format!("{WS}/ops/FOREIGN_5L_LIFT.held"), "key": "held"},
                {"path": format!("{WS}/ops/FOREIGN_5L_LIFT.json"), "key": "all_cpus_listed"},
                {"path": format!("{WS}/ops/OUR_PINS.json"), "key": "entries[].cpus"}],
            "pause_files": [format!("{WS}/ops/PAUSE"), format!("{WS}/plan/auto-campaign/implementer_ops/PAUSE")],
            "idle_min": 0.85, "max_cores": 8, "respect_foreign_pinning": true, "ignore_own_tag_claims": true
        })
        .to_string(),
    )
    .unwrap();
    let mut v = spec(
        &t,
        serde_json::json!({"items": 16, "nodes": 2, "rungs": 1, "seed": 1, "case_ms": [1000, 2000],
            "p_norule": 0.25, "p_mem": 0.0, "p_crash": 0.0, "deps": 0.3, "mem_cap_mb": 48}),
        serde_json::json!({"max_jobs": 8, "mem_total": "8G", "host_floor": "50G", "disk_floor": "5G",
            "grace_seconds": 20, "sampler_ms": 500}),
    );
    let res = v["resources"].as_object_mut().unwrap();
    res.remove("cpus");
    res.insert("policy".into(), serde_json::json!("site"));
    res.insert("site_policy".into(), serde_json::json!(site));
    let sp = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let t0 = Instant::now();
    let code = run_env(&sp, &d, &[]);
    let started = count(&d, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Started { .. },
                ..
            }
        )
    });
    let text = std::fs::read_to_string(&ledger).unwrap();
    let mut held_now: BTreeSet<usize> = BTreeSet::new();
    let (mut claims, mut releases, mut peak) = (0usize, 0usize, 0usize);
    let mut claimed_all: BTreeSet<usize> = BTreeSet::new();
    for l in text.lines() {
        if let Some(i) = l.find("): + CPU") {
            let rest = &l[i + 3..];
            let list = rest
                .trim_start_matches("+ CPUs ")
                .trim_start_matches("+ CPU ")
                .split_whitespace()
                .next()
                .unwrap_or("");
            let set = parse_list(list);
            claims += 1;
            claimed_all.extend(set.iter().copied());
            held_now.extend(set);
            peak = peak.max(held_now.len());
        } else if let Some(i) = l.find("): RELEASED ") {
            let list = l[i + 12..].split_whitespace().next().unwrap_or("");
            for c in parse_list(list) {
                held_now.remove(&c);
            }
            releases += 1;
        }
    }
    let excluded: BTreeSet<usize> = RUNBOOK_EXCLUDE.iter().flat_map(|e| parse_list(e)).collect();
    eprintln!(
        "PF11 runbook site.json: exit {code} after {:?}, {started} jobs started, {claims} claims / {releases} releases, peak {peak} CPUs at once, CPUs used {}",
        t0.elapsed(),
        rustred_campaign::resources::format_cpu_list(&claimed_all)
    );
    assert_eq!(code, 0, "{}", log_tail(&d));
    assert!(started >= 16, "{started} jobs: {}", log_tail(&d));
    assert!(
        claimed_all.is_disjoint(&excluded),
        "a reserved CPU was claimed: {:?}",
        claimed_all.intersection(&excluded).collect::<Vec<_>>()
    );
    assert!(peak <= 8, "{peak} CPUs at once (max_cores 8)");
    assert!(
        held_now.is_empty() && claims == releases,
        "unreleased claims {held_now:?}"
    );
}

/// M11-2: a site policy that can NEVER grant a CPU (no candidate CPU has its lock file) is a TYPED refusal at start
/// (exit 1, "cpu policy: nothing can ever be granted"), never a silent wait; nothing is launched.
#[test]
fn m11_2_a_site_policy_that_can_never_grant_is_refused_at_start() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf11-never");
    let locks = t.path.join("locks");
    std::fs::create_dir(&locks).unwrap(); // no cpu.N file at all
    let ledger = t.path.join("CPU_LEDGER.md");
    std::fs::write(&ledger, "").unwrap();
    let site = t.path.join("site.json");
    std::fs::write(
        &site,
        serde_json::json!({
            "schema": "rustred.campaign.site-policy.v1", "tag": "rr-campaign",
            "lock_dir": locks, "lock_name": "cpu.{n}", "ledger_file": ledger, "candidates": null,
            "exclude": [], "held_files": [], "pause_files": [], "idle_min": 0.0, "max_cores": 2,
            "respect_foreign_pinning": false
        })
        .to_string(),
    )
    .unwrap();
    let mut v = spec(
        &t,
        serde_json::json!({"items": 2, "case_ms": [200, 400]}),
        serde_json::json!({}),
    );
    let res = v["resources"].as_object_mut().unwrap();
    res.remove("cpus");
    res.insert("policy".into(), serde_json::json!("site"));
    res.insert("site_policy".into(), serde_json::json!(site));
    let sp = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let t0 = Instant::now();
    let code = run_env(&sp, &d, &[]);
    eprintln!("PF11 never admits: exit {code} after {:?}", t0.elapsed());
    assert_eq!(code, 1, "{}", log_tail(&d));
    assert!(
        log_tail(&d).contains("nothing can ever be granted"),
        "{}",
        log_tail(&d)
    );
    assert!(t0.elapsed() < Duration::from_secs(120));
}

/// m11-mut M9 (gate10 m10-9): a supervisor that is NOT a child subreaper (test-build failpoint `no_subreaper`) has no
/// atomic proof of its own tree: a healthy campaign ends 8 ("not a child subreaper"), never a claimed 0.
#[test]
fn m11_mut_m9_a_supervisor_that_is_not_a_subreaper_never_claims_the_end() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf11-nosr");
    let v = spec(
        &t,
        serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "case_ms": [200, 400], "p_crash": 0.0,
            "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"unproven_end_limit_s": 30}),
    );
    let sp = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let code = run_env(&sp, &d, &[("RR_CAMPAIGN_FAILPOINT", "no_subreaper:1")]);
    eprintln!("PF11 no subreaper: exit {code}");
    assert_eq!(code, ATTENTION, "{}", log_tail(&d));
    assert!(
        log_tail(&d).contains("not a child subreaper"),
        "{}",
        log_tail(&d)
    );
    // control: the same campaign with the subreaper ends 0
    let t2 = Tmp::new("pf11-sr");
    let v2 = spec(
        &t2,
        serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "case_ms": [200, 400], "p_crash": 0.0,
            "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"unproven_end_limit_s": 30}),
    );
    let sp2 = write_spec(&t2, "spec.json", &v2);
    let d2 = t2.cdir("c");
    assert_eq!(run_env(&sp2, &d2, &[]), 0, "{}", log_tail(&d2));
}

/// The preload of the pid-reuse test: the pidfd_open of exactly `PF11_PID` (the first one: the sweep's stop of that
/// candidate; nothing earlier opens a pidfd of another process) appends a line to `PF11_HIT` and is then DELAYED
/// `PF11_DELAY_US` (a descheduled supervisor between the marker read and the pidfd, as gate10's g10fault.so delay).
const PIDFD_DELAY_C: &str = r#"#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdlib.h>
#include <sys/syscall.h>
#include <unistd.h>
long syscall(long n, ...) {
  static long (*r)(long, ...) = 0;
  if (!r) r = (long (*)(long, ...))dlsym(RTLD_NEXT, "syscall");
  va_list ap; va_start(ap, n);
  long a1 = va_arg(ap, long), a2 = va_arg(ap, long), a3 = va_arg(ap, long);
  long a4 = va_arg(ap, long), a5 = va_arg(ap, long), a6 = va_arg(ap, long);
  va_end(ap);
#ifdef SYS_pidfd_open
  if (n == SYS_pidfd_open) {
    const char *t = getenv("PF11_PID"), *f = getenv("PF11_HIT"), *d = getenv("PF11_DELAY_US");
    if (t && atol(t) == a1) {
      if (f) { int fd = open(f, O_WRONLY | O_CREAT | O_APPEND, 0600); if (fd >= 0) { (void)!write(fd, "hit\n", 4); close(fd); } }
      usleep(d ? atoi(d) : 400000);
    }
  }
#endif
  return r(n, a1, a2, a3, a4, a5, a6);
}
"#;

/// m11-mut M4 (gate10's pid-reuse scenario as a test): a crashed session's marked daemon M (pid P); session 2 runs with
/// the pidfd_open of P DELAYED 400 ms (the preload above). When session 2's recovery sweep opens P's pidfd, the test
/// SIGKILLs M, reaps it (the test is the namespace's init), sets ns_last_pid = P - 1 and starts an UNMARKED decoy D that
/// receives P. The sweep's stop then meets another process at P (StartMismatch): the marked incarnation vanished
/// unproven (exit 8, note sweep_vanished), and D is NEVER signalled. Runs until one run reuses the pid (at most 4).
#[test]
fn m11_mut_m4_a_pid_reused_before_the_stop_is_vanished_and_never_signalled() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let bins = Tmp::new("pf11-reuse-bin");
    let src = bins.path.join("pidfd_delay.c");
    std::fs::write(&src, PIDFD_DELAY_C).unwrap();
    let fault = bins.path.join("pidfd_delay.so");
    // no C compiler: skipped; a compile ERROR is a failure of this test (never a silent skip)
    match Command::new("cc")
        .args(["-shared", "-fPIC", "-O1", "-o"])
        .arg(&fault)
        .arg(&src)
        .arg("-ldl")
        .env("PATH", path_env())
        .output()
    {
        Err(e) => {
            eprintln!("SKIPPED: no C compiler ({e})");
            return;
        }
        Ok(o) => assert!(
            o.status.success(),
            "the preload does not compile: {}",
            String::from_utf8_lossy(&o.stderr)
        ),
    }
    let mut reused = 0;
    for i in 0..4 {
        let t = Tmp::new(&format!("pf11-reuse-{i}"));
        let dpf = t.path.join("daemon.pid");
        let sp = spec_once(
            &t,
            &format!(
                "( setsid sleep 600 </dev/null >/dev/null 2>&1 & echo $! > {p}.tmp; mv {p}.tmp {p} )",
                p = dpf.display()
            ),
            serde_json::json!({}),
        );
        let d = t.cdir("c");
        let mut s1 = start(&sp, &d, &["--plain"], &[]);
        wait_until("the daemon", Duration::from_secs(300), || dpf.exists());
        std::thread::sleep(Duration::from_millis(500));
        signal(&s1, libc::SIGKILL);
        let _ = s1.wait();
        let dp: u32 = std::fs::read_to_string(&dpf)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let hit = t.path.join("pidfd.hit");
        let so = fault.to_str().unwrap().to_string();
        let hs = hit.to_str().unwrap().to_string();
        let dps = dp.to_string();
        let mut s2 = start(
            &sp,
            &d,
            &["--plain"],
            &[
                ("LD_PRELOAD", &so),
                ("PF11_PID", &dps),
                ("PF11_HIT", &hs),
                ("PF11_DELAY_US", "400000"),
            ],
        );
        let t0 = Instant::now();
        while !hit.exists() && t0.elapsed() < Duration::from_secs(300) {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            hit.exists(),
            "the recovery sweep never opened the daemon's pidfd: {}",
            log_tail(&d)
        );
        kill_pid(dp);
        let t1 = Instant::now();
        while std::path::Path::new(&format!("/proc/{dp}")).exists()
            && t1.elapsed() < Duration::from_secs(2)
        {
            reap(dp);
            std::thread::sleep(Duration::from_millis(2));
        }
        let _ = std::fs::write("/proc/sys/kernel/ns_last_pid", format!("{}", dp - 1));
        let decoy = Command::new("sleep")
            .arg("600")
            .env_clear()
            .env("PATH", path_env())
            .spawn()
            .unwrap();
        let dpid = decoy.id();
        let dstart = start_ticks(dpid);
        let code = wait(&mut s2, Duration::from_secs(900));
        let d_alive = alive(dpid) && start_ticks(dpid) == dstart;
        let signalled = count(
            &d,
            |e| matches!(e, Event::SignalSent { pid, .. } if *pid == dpid),
        );
        let vanished = notes(&d, "sweep_vanished {");
        eprintln!(
            "PF11 pid reuse {i}: daemon {dp}, decoy {dpid} (reuse {}), exit {code}, decoy alive {d_alive}, signal_sent to it {signalled}, vanished notes {}",
            dpid == dp,
            vanished.len()
        );
        kill_pid(dpid);
        let mut decoy = decoy;
        let _ = decoy.wait();
        assert!(
            d_alive,
            "the decoy at a reused pid was signalled or is gone"
        );
        assert_eq!(signalled, 0, "a signal_sent record names the decoy");
        if dpid == dp {
            reused += 1;
            assert_ne!(
                code,
                0,
                "exit 0 although the marked process at a reused pid vanished unproven: {}",
                log_tail(&d)
            );
            assert!(
                vanished.iter().any(|n| n.contains(&dp.to_string())),
                "the reused pid is recorded as vanished: {vanished:?} {}",
                log_tail(&d)
            );
            break;
        }
    }
    assert!(reused >= 1, "no run reused the daemon's pid (4 tries)");
}
