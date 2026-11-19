//! Lane procfix7 round 16 (gate17, notes/implementer.gate17.md): `rr_campaign units clear` after a supervisor CRASH
//! (m17-units-takeover). The crashed supervisor's lock record (LOCK.json, its owner dead) is the evidence the next start
//! turns into a `lock_takeover` record (and its token orphans into recovery's seeds). Since round 15 `units clear` takes
//! the campaign lock; before this round it TOOK OVER the stale lock and its release REMOVED LOCK.json, so the restart saw
//! a fresh lock (gate 17: lock_takeover records 1 without `units clear`, 0 with it). Now `units clear` refuses (exit 1)
//! and leaves the crashed supervisor's record in place.
//!
//! Spawns processes and sends signals: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use std::process::{Command, Stdio};
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

fn owner_pid(d: &std::path::Path) -> Option<u32> {
    let b = std::fs::read(d.join("LOCK.json")).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&b).ok()?;
    // OwnerRecord flattens its identity: the pid is a top-level field of LOCK.json
    v["pid"].as_u64().map(|p| p as u32)
}

#[test]
fn units_clear_after_a_crash_refuses_and_leaves_the_stale_lock_to_the_restart() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf16-units");
    let v = spec(
        &t,
        serde_json::json!({"items": 4, "nodes": 1, "rungs": 1, "seed": 3, "case_ms": [15000, 20000]}),
        serde_json::json!({"max_jobs": 2}),
    );
    let sp = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let mut c = start(&sp, &d, &[], &[]);
    let crashed = c.id();
    wait_until("a job started", Duration::from_secs(180), || {
        d.join("ledger").exists()
            && count(&d, |e| {
                matches!(
                    e,
                    Event::Job {
                        ev: JobEvent::Started { .. },
                        ..
                    }
                )
            }) > 0
    });
    // the crash: SIGKILL of OUR OWN supervisor (our child): its lock record stays, its owner dead
    signal(&c, libc::SIGKILL);
    c.wait().unwrap();
    assert_eq!(
        owner_pid(&d),
        Some(crashed),
        "the crashed supervisor's record"
    );
    let o = Command::new(SUP)
        .arg("units")
        .arg("clear")
        .arg(&d)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    eprintln!("PF16 units clear: exit {:?}; {err}", o.status.code());
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(
        err.contains("refused") && err.contains("did not end cleanly"),
        "{err}"
    );
    // the evidence stays: the next start must see the stale lock
    assert_eq!(
        owner_pid(&d),
        Some(crashed),
        "LOCK.json must still name the crashed supervisor"
    );
    let mut c2 = start(&sp, &d, &[], &[]);
    let code = wait(&mut c2, Duration::from_secs(600));
    let takeovers: Vec<u32> = records(&d)
        .into_iter()
        .filter_map(|r| match r.event {
            Event::LockTakeover { previous, .. } => Some(previous.identity.pid),
            _ => None,
        })
        .collect();
    eprintln!("PF16 restart: exit {code}; lock_takeover of {takeovers:?}");
    kill_marked(&t.campaign);
    assert_eq!(takeovers, vec![crashed], "{}", log_tail(&d));
    assert_eq!(verify(&d), 0);
}
