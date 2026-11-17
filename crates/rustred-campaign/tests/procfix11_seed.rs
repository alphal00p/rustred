//! Lane procfix7 round 7 (gate11 m11-mut, mutation M6: the sweeps' SEEDS never checked by their own pid): a live MARKED
//! process that no listing and no completeness walk shows (test-build procfs fault `proc_hide_pid:<pid>`: missing from
//! both /proc enumerations AND from every children list) is found ONLY through a seed (a pid the ledger recorded):
//! `launch::sweep::sweep_seeded` with that pid as seed stops and kills it; without the seed (control) the same sweep
//! sees nothing. A build that ignores the seeds leaves it alive here.
//!
//! Its own test binary: the fault layer reads RR_CAMPAIGN_FAILPOINT once per process, set here before the crate's first
//! /proc read. Spawns and signals a process of its own: ONLY inside a private pid namespace (`LIGHT_PIDNS=1`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::launch::sweep::{SweepSpec, sweep_seeded};
use rustred_campaign::resources::MARKER_VAR;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| s.rfind(')').map(|i| !s[i + 2..].starts_with('Z')))
        .unwrap_or(false)
}

#[test]
fn m11_mut_m6_a_seed_finds_the_process_no_listing_shows() {
    if !rustred_campaign::launch::in_private_namespace() {
        eprintln!(
            "SKIPPED: spawns and signals a process: only inside a private pid namespace (rule L10)"
        );
        return;
    }
    let id = "pf11seed.0123456789abcdef";
    let mut child = Command::new("sleep")
        .arg("600")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env(MARKER_VAR, format!("{id}:job-t0:a1"))
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let pid = child.id();
    // test code reads /proc directly: wait for the exec (the marker in its environ), then let it age past the
    // listing's 2-tick slack
    let t0 = Instant::now();
    while !std::fs::read(format!("/proc/{pid}/environ")).is_ok_and(|b| {
        b.windows(MARKER_VAR.len())
            .any(|w| w == MARKER_VAR.as_bytes())
    }) {
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "the child never exec'd"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(200));
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let start: u64 = stat[stat.rfind(')').unwrap() + 2..]
        .split_whitespace()
        .nth(19)
        .unwrap()
        .parse()
        .unwrap();
    // SAFETY: set before the crate's first /proc read in this process (one test in this binary: no concurrent reader)
    unsafe { std::env::set_var("RR_CAMPAIGN_FAILPOINT", format!("proc_hide_pid:{pid}")) };
    let spec = SweepSpec {
        campaign_id: id.into(),
        only_attempt: None,
        not_before_ticks: 0,
        budget: Duration::from_secs(20),
        pass_gap: Duration::from_millis(50),
        narrowing: None,
    };
    // control: no seed: the hidden process is not seen (nothing killed, it lives)
    let r0 = sweep_seeded(&spec, &[], &AtomicBool::new(false));
    eprintln!(
        "PF11 seed control: proven {} killed {} seen {}",
        r0.proven,
        r0.killed.len(),
        r0.seen
    );
    assert!(r0.killed.is_empty(), "{r0:?}");
    assert!(alive(pid), "precondition: hidden, not found, alive");
    // the seed: found by its own pid, stopped and killed
    let r = sweep_seeded(&spec, &[(pid, Some(start))], &AtomicBool::new(false));
    eprintln!(
        "PF11 seed: proven {} killed {:?} seen {}",
        r.proven,
        r.killed.iter().map(|k| k.pid).collect::<Vec<_>>(),
        r.seen
    );
    // bounded: a sweep that did not kill it must fail here, never hang on the child
    let t1 = Instant::now();
    let st = loop {
        if let Some(st) = child.try_wait().unwrap() {
            break Some(st);
        }
        if t1.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            st.and_then(|s| s.signal()),
            Some(libc::SIGKILL),
            "the seeded sweep killed the hidden marked process: {r:?}"
        );
    }
    assert!(r.killed.iter().any(|k| k.pid == pid), "{r:?}");
}
