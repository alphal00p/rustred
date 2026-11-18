//! Lane procfix7 round 14 (host step 3, WS/plan/auto-campaign/reports/HOST-STEP-3-ANALYSIS.md): the USER MANAGER as a
//! host condition (M2 pending state, M3 manager gate, M4 leftover list, M1 stop decision with the ledger alive).
//!
//! Inside a private namespace there is no systemd: the test-build failpoint `manager_stall:1` emulates scope launches
//! against a user manager that STALLS while the file named by `RR_TEST_MANAGER_STALL` exists: every job requests the unit
//! `rr-fp-stall`; while the file exists its containment is `Pending` and its launch state `Pending` (the kernel decision of
//! M2, `launch::launch_state_from`, is unit-tested on recorded inputs in launch/real.rs); the supervisor's manager probe
//! answers exactly when the file does not exist. Cases:
//! - a stall of ~15 s, longer than the 10-s rule: no attempt is killed or counted, the gate closes (note
//!   `manager_unresponsive`), NO launch happens while it is closed, it reopens (note `manager_answered`) when the stall
//!   ends, and the campaign finishes every item;
//! - a stall longer than `resources.manager_wait_limit_s`: a typed stop, exit 69 (note `stop_decided`), the pending
//!   launchers ended through their pinned groups and their unit on the leftover list `units.leftover.json`, and every
//!   delivered signal and every end recorded in the ledger.
//!
//! Spawns processes and sends signals: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use std::path::{Path, PathBuf};
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

fn spec_for(t: &Tmp, items: u64, case_ms: u64, wait_limit_s: u64) -> PathBuf {
    let v = spec(
        t,
        serde_json::json!({"items": items, "nodes": 1, "rungs": 1, "seed": 3, "case_ms": [case_ms, case_ms],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 2, "scope_pending_warn_s": 2, "manager_wait_limit_s": wait_limit_s}),
    );
    write_spec(t, "spec.json", &v)
}

/// The ledger seq of every record whose note text starts with `prefix`.
fn note_seqs(d: &Path, prefix: &str) -> Vec<u64> {
    records(d)
        .into_iter()
        .filter_map(|r| match &r.event {
            Event::Note { text } if text.starts_with(prefix) => Some(r.seq),
            _ => None,
        })
        .collect()
}

fn launch_seqs(d: &Path) -> Vec<u64> {
    records(d)
        .into_iter()
        .filter_map(|r| match &r.event {
            Event::Job {
                ev: JobEvent::Launch { .. },
                ..
            } => Some(r.seq),
            _ => None,
        })
        .collect()
}

/// Every delivered intent of the intent log has its ledger `signal_sent`; every launch its `finished` record; the log
/// never says that a delivered signal was not recorded (M1).
fn ledger_complete(d: &Path) {
    let log = std::fs::read_to_string(d.with_extension("log")).unwrap_or_default();
    assert!(
        !log.contains("not recorded") && !log.contains("ledger is unavailable"),
        "{}",
        log_tail(d)
    );
    let intents = std::fs::read_to_string(d.join("log/signal-intents.jsonl")).unwrap_or_default();
    let delivered: Vec<String> = intents
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["kind"] == "done" && v["result"] == "delivered")
        .filter_map(|v| v["id"].as_str().map(str::to_string))
        .collect();
    let bys: Vec<String> = records(d)
        .into_iter()
        .filter_map(|r| match r.event {
            Event::SignalSent { by, .. } => Some(by),
            _ => None,
        })
        .collect();
    let missing: Vec<&String> = delivered
        .iter()
        .filter(|id| !bys.iter().any(|b| b.contains(&format!("[intent {id}]"))))
        .collect();
    assert!(missing.is_empty(), "delivered, not recorded: {missing:?}");
    let launches = launch_seqs(d).len();
    let finished = count(d, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Finished { .. },
                ..
            }
        )
    });
    assert_eq!(launches, finished, "every launch has its end recorded");
}

#[test]
fn a_manager_stall_longer_than_the_10s_rule_pauses_launches_kills_nothing_and_resumes() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf15-stall");
    let stall = t.path.join("manager.stalled");
    std::fs::write(&stall, "").unwrap();
    let sp = spec_for(&t, 4, 30_000, 600);
    let d = t.cdir("c");
    let mut c = start(
        &sp,
        &d,
        &[],
        &[
            ("RR_CAMPAIGN_FAILPOINT", "manager_stall:1"),
            ("RR_TEST_MANAGER_STALL", stall.to_str().unwrap()),
        ],
    );
    // the gate closes while the first launches are pending; the stall outlasts the 10-s rule
    wait_until("the gate closed", Duration::from_secs(120), || {
        d.join("ledger").exists() && !note_seqs(&d, "manager_unresponsive ").is_empty()
    });
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(200));
    }
    std::fs::remove_file(&stall).unwrap();
    let code = wait(&mut c, Duration::from_secs(600));
    eprintln!(
        "PF15 stall: exit {code}; {}",
        log_tail(&d)
            .lines()
            .filter(|l| l.contains("manager") || l.contains("scope"))
            .take(10)
            .collect::<Vec<_>>()
            .join(" | ")
    );
    kill_marked(&t.campaign);
    assert_eq!(code, finish_code_by_items(&d), "{}", log_tail(&d));
    assert_eq!(code, 0, "every item closed: {}", log_tail(&d));
    // no attempt killed or counted for its scope while it waited for the manager
    assert!(notes(&d, "scope_unverified_limit ").is_empty());
    assert!(
        !notes(&d, "job_containment ")
            .iter()
            .any(|n| n.contains("scope_unverified_ended")),
        "{:?}",
        notes(&d, "job_containment ")
    );
    // closed, then opened again, and no launch while closed
    let closed = note_seqs(&d, "manager_unresponsive ");
    let opened = note_seqs(&d, "manager_answered ");
    assert!(
        !closed.is_empty() && !opened.is_empty(),
        "{closed:?} {opened:?}"
    );
    let (c0, o0) = (closed[0], *opened.iter().find(|&&o| o > closed[0]).unwrap());
    let inside: Vec<u64> = launch_seqs(&d)
        .into_iter()
        .filter(|&l| l > c0 && l < o0)
        .collect();
    assert!(
        inside.is_empty(),
        "launches while the gate was closed: {inside:?}"
    );
    assert!(launch_seqs(&d).iter().any(|&l| l > o0), "launches resumed");
    ledger_complete(&d);
}

#[test]
fn a_manager_stall_beyond_the_wait_limit_is_a_typed_stop_with_the_leftovers_listed() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf15-limit");
    let stall = t.path.join("manager.stalled");
    std::fs::write(&stall, "").unwrap();
    let sp = spec_for(&t, 4, 60_000, 10);
    let d = t.cdir("c");
    let mut c = start(
        &sp,
        &d,
        &[],
        &[
            ("RR_CAMPAIGN_FAILPOINT", "manager_stall:1"),
            ("RR_TEST_MANAGER_STALL", stall.to_str().unwrap()),
        ],
    );
    let code = wait(&mut c, Duration::from_secs(600));
    eprintln!(
        "PF15 limit: exit {code}; {}",
        log_tail(&d)
            .lines()
            .filter(|l| l.contains("manager") || l.contains("leftover"))
            .take(10)
            .collect::<Vec<_>>()
            .join(" | ")
    );
    kill_marked(&t.campaign);
    assert_eq!(code, 69, "manager unavailable: {}", log_tail(&d));
    let dec = notes(&d, "stop_decided ");
    assert_eq!(dec.len(), 1, "{dec:?}");
    assert!(dec[0].contains("\"exit\":69"), "{dec:?}");
    assert!(!notes(&d, "manager_unresponsive ").is_empty());
    assert!(!notes(&d, "unit_leftover ").is_empty());
    let list = std::fs::read_to_string(d.join("units.leftover.json")).unwrap();
    assert!(list.contains("rr-fp-stall"), "{list}");
    assert!(notes(&d, "scope_unverified_limit ").is_empty());
    ledger_complete(&d);
}
