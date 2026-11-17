//! Lane procfix7 round 10 (gate13c G13c-M1, rb_c2 made deterministic): the end-of-job proof must NEVER report Verified
//! while a re-forking descendant of the job is alive.
//!
//! The race: the proof lists our children (and /proc, and the marker scan), then examines each entry; a relay hop that
//! handed off to a child born after the listing is found DEAD or GONE when examined, and before round 10 that counted as
//! absence. The test-build hook `resources::set_listing_hold(ms, n)` (not compiled into release builds) pauses `ms`
//! between each of the next `n` listings and the reads of its entries: a relay hopping every 1 ms is then always gone or
//! dead when its listed entry is read. Before the fix this reported Verified{ProcTwoPass, SubreaperTree} while the relay
//! ran, every time; after it, the hunt keeps looking (a listed child gone or dead is a CHANGE, a proof pass that sees one
//! is INCONCLUSIVE) and the relay ends killed, Verified only after it is gone, or Unverified and then cleared by the
//! quarantine.
//!
//! Spawns and signals processes: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::launch::{
    CapPreference, ChildHandle, ChildLauncher, ChildProgress, FinalReport, RealChild, RealConfig,
    RealLauncher, RetireEvent, TrackingPreference,
};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

fn launcher() -> RealLauncher {
    RealLauncher::with_config(
        sampler(),
        RealConfig {
            cap_preference: CapPreference::NoSystemd,
            tracking: TrackingPreference::ProcOnly,
            verify_timeout: Duration::from_secs(10),
            rehunt_every: Duration::from_millis(300),
            ..RealConfig::default()
        },
    )
}

fn poll_until_finished(h: &mut RealChild, t: Duration) -> Option<FinalReport> {
    let t0 = Instant::now();
    while t0.elapsed() < t {
        if let Ok(ChildProgress::Finished(r)) = h.poll() {
            return Some(r);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

/// Helper entry for re-executed modes (does nothing in a normal run).
#[test]
fn helper_entry() {
    if std::env::var("RR_PROC_HELPER").as_deref() == Ok("relay") {
        relay_helper()
    }
}

/// The relay of rb_c2 (proc_reaudit_regressions): fork, the parent exits, the child calls setsid and repeats every
/// `pause` us (async-signal-safe calls only in the forked child); the job's leader exits at once.
fn relay_helper() {
    let pause: u32 = std::env::var("RR_RELAY_PAUSE_US")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    let counter = std::env::var("RR_RELAY_COUNTER").unwrap();
    let c = std::ffi::CString::new(counter).unwrap();
    // SAFETY: open with a valid C string.
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_WRONLY | libc::O_CREAT, 0o600) };
    assert!(fd >= 0);
    // SAFETY: fork; the child uses only async-signal-safe calls until it exits.
    let pid = unsafe { libc::fork() };
    if pid == 0 {
        let mut i: u64 = 0;
        loop {
            // SAFETY: as above (single-threaded child).
            unsafe {
                if libc::fork() != 0 {
                    libc::_exit(0);
                }
                libc::setsid();
                i += 1;
                let b = i.to_le_bytes();
                libc::pwrite(fd, b.as_ptr().cast(), 8, 0);
                libc::usleep(pause);
            }
        }
    }
}

fn relay_count(p: &Path) -> u64 {
    fs::read(p)
        .ok()
        .filter(|b| b.len() >= 8)
        .map(|b| u64::from_le_bytes(b[..8].try_into().unwrap()))
        .unwrap_or(0)
}

#[test]
fn g13c_m1_a_handed_off_relay_is_never_verified_while_it_runs() {
    if std::process::id() != 1 {
        eprintln!(
            "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
        );
        return;
    }
    let _g = serial();
    let dir = TempDir::new("pf14-handoff");
    let mut l = launcher();
    let cf = dir.path().join("relay.count");
    let mut s = helper_spec(dir.path(), "relayH", "helper_entry", "relay");
    s.env.push(("RR_RELAY_PAUSE_US".into(), "1000".into()));
    s.env.push(("RR_RELAY_COUNTER".into(), cf.clone().into()));
    // every listing of the next 60 is read 30 ms late: each listed hop (1 ms apart) is gone or dead when examined
    rustred_campaign::resources::set_listing_hold(30, 60);
    let t0 = Instant::now();
    let mut h = l.launch(&s).unwrap();
    let r = poll_until_finished(&mut h, Duration::from_secs(90)).expect("a report");
    let c1 = relay_count(&cf);
    std::thread::sleep(Duration::from_secs(1));
    let c2 = relay_count(&cf);
    eprintln!(
        "PF14 report after {:?}: {:?}; relay hops {c1} -> {c2}",
        t0.elapsed(),
        r.verification
    );
    assert!(
        !(r.verification.is_verified() && c2 > c1),
        "G13c-M1: Verified while the relay runs ({c1} -> {c2} hops): {r:?}"
    );
    if !r.verification.is_verified() {
        // the quarantine keeps hunting (the hold is used up by now): the relay is caught and the end proven
        assert!(l.retire(h, None).is_none());
        let t1 = Instant::now();
        loop {
            let cleared = l
                .tick()
                .into_iter()
                .any(|ev| matches!(&ev, RetireEvent::Cleared { label, .. } if label == "relayH"));
            if cleared {
                break;
            }
            assert!(
                t1.elapsed() < Duration::from_secs(120),
                "not cleared within 120 s: {:?}",
                l.quarantined()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        eprintln!("PF14 cleared by the quarantine after {:?}", t1.elapsed());
    }
    let a = relay_count(&cf);
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(a, relay_count(&cf), "the relay still runs");
}
