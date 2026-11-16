//! Operator-facing behaviour: pause flag honoured (no launch while it exists), control requests applied and logged
//! (pause, resume, skip / retry, stop -> exit 140), pre-flight refusals each with its exit code, the heartbeat timeout
//! killing a silent child (class killed_time, the item escalates), and memory-cap kills (killed_memory) with the
//! admission budgets read back from the ledger.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use rustred_campaign::state::{FinishClass, ItemPhase};
use std::time::Duration;

fn launches(d: &std::path::Path) -> usize {
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

#[test]
fn pause_flag_blocks_launching() {
    let _g = serial();
    let t = Tmp::new("pause");
    let flag = t.path.join("PAUSE");
    std::fs::write(&flag, "").unwrap();
    let res = serde_json::json!({"pause_files": [flag]});
    let syn = serde_json::json!({"items": 6, "case_ms": [80, 150]});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, res));
    let d = t.cdir("run");
    let mut c = start(&spec_p, &d, &[], &[]);
    wait_until("ledger", Duration::from_secs(60), || {
        d.join("ledger").exists()
    });
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(launches(&d), 0, "launched while the pause flag exists");
    // An IDLE live supervisor (nothing launches, the ledger does not move) still rewrites heartbeat.json at least
    // every 10 s, so no reader ever sees it STALE.
    let hb = || {
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(d.join("progress/heartbeat.json")).unwrap())
                .unwrap();
        v["supervisor"]["heartbeat_utc"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let h0 = hb();
    let seq0 = state(&d).last_seq;
    wait_until(
        "heartbeat advances while idle",
        Duration::from_secs(10),
        || hb() != h0,
    );
    assert_eq!(state(&d).last_seq, seq0, "idle: no ledger record");
    assert!(
        log_tail(&d).contains("launching paused"),
        "{}",
        log_tail(&d)
    );
    std::fs::remove_file(&flag).unwrap();
    // procfix5 (gate4 m3/m4): 10 when a synthetic item crashed twice (attention)
    let code = wait(&mut c, Duration::from_secs(300));
    assert_eq!(code, finish_code_by_items(&d), "{}", log_tail(&d));
    assert!(launches(&d) > 0);
    assert_no_marked(&t.campaign);
}

#[test]
fn control_requests_are_applied_and_logged() {
    let _g = serial();
    let t = Tmp::new("ctl");
    let syn = serde_json::json!({"items": 20, "case_ms": [300, 600], "deps": 0.0, "p_crash": 0.0, "p_mem": 0.0});
    let spec_p = write_spec(
        &t,
        "spec.json",
        &spec(&t, syn, serde_json::json!({"max_jobs": 2})),
    );
    let d = t.cdir("run");
    let ds = d.to_string_lossy().into_owned();
    let mut c = start(&spec_p, &d, &[], &[]);
    wait_until("ledger", Duration::from_secs(60), || {
        d.join("ledger").exists()
    });
    // skip an item far down the queue (open), then pause.
    let last = "N0.i0018.L0";
    assert_eq!(cli(&["control", &ds, "skip", last]).0, 0);
    assert_eq!(cli(&["control", &ds, "pause"]).0, 0);
    let has = |verb: &str, status: Option<&str>| {
        controls(&d)
            .iter()
            .any(|(_, v, s)| v == verb && status.is_none_or(|w| w == s) && s != "requested")
    };
    wait_until("pause applied", Duration::from_secs(60), || {
        has("pause", Some("applied"))
    });
    std::thread::sleep(Duration::from_millis(500));
    let n0 = launches(&d);
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(launches(&d), n0, "launched while paused");
    assert_eq!(cli(&["control", &ds, "resume"]).0, 0);
    wait_until("skip applied or refused", Duration::from_secs(60), || {
        has("skip", None)
    });
    // retry the skipped item (ledger: skipped -> open, new generation, not a failure).
    assert_eq!(cli(&["control", &ds, "retry", last]).0, 0);
    wait_until("retry applied or refused", Duration::from_secs(60), || {
        has("retry", None)
    });
    assert_eq!(cli(&["control", &ds, "stop"]).0, 0);
    assert_eq!(
        wait(&mut c, Duration::from_secs(300)),
        140,
        "{}",
        log_tail(&d)
    );
    // Every control is a ledger record (control requested -> applied / refused), never a note.
    let applied = controls(&d);
    for v in ["pause", "resume", "stop", "skip", "retry"] {
        assert!(
            applied.iter().any(|(_, w, s)| w == v && s == "applied"),
            "{v}: {applied:?}"
        );
    }
    assert!(
        notes(&d, "control_").is_empty(),
        "{:?}",
        notes(&d, "control_")
    );
    assert!(std::fs::read_dir(d.join("control/done")).unwrap().count() >= 5);
    // Unknown campaign directory / usage errors.
    assert_eq!(
        cli(&["control", &t.path.join("nope").to_string_lossy(), "pause"]).0,
        3
    );
    assert_eq!(cli(&["control", &ds, "skip"]).0, 2);
    // Resume finishes the campaign (the skip, if applied, was retried).
    run_to_end(&spec_p, &d, &[]);
    let st = state(&d);
    assert!(!matches!(st.items[last].phase, ItemPhase::Skipped));
    assert_eq!(st.items[last].operator_retries, 1);
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

#[test]
fn preflight_refusals_have_distinct_codes() {
    let _g = serial();
    let t = Tmp::new("pf");
    let d = t.cdir("run");
    let ds = d.to_string_lossy().into_owned();
    let base = spec(&t, serde_json::json!({"items": 4}), serde_json::json!({}));
    let pf = |name: &str, v: &serde_json::Value| {
        let p = write_spec(&t, name, v);
        cli(&["preflight", "--spec", &p.to_string_lossy(), "--dir", &ds])
    };
    let (code, out) = pf("ok.json", &base);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("ready"));
    let mut v = base.clone();
    v["schema"] = "nope".into();
    assert_eq!(pf("bad.json", &v).0, 20);
    let mut v = base.clone();
    v["programs"]["rr_fake_child"]["path"] = "/nonexistent/rr_fake_child".into();
    assert_eq!(pf("prog.json", &v).0, 21);
    let mut v = base.clone();
    v["licence"] = serde_json::json!({"env": "RR_SUP_TEST_SURELY_UNSET_VARIABLE"});
    let (code, out) = pf("lic.json", &v);
    assert_eq!(code, 22);
    assert!(out.contains("RR_SUP_TEST_SURELY_UNSET_VARIABLE"));
    let mut v = base.clone();
    v["resources"]["host_floor"] = "100T".into();
    assert_eq!(pf("mem.json", &v).0, 23);
    let mut v = base.clone();
    v["resources"]["disk_floor"] = "1000T".into();
    assert_eq!(pf("disk.json", &v).0, 24);
    let mut v = base.clone();
    let input = t.path.join("table.jsonl");
    std::fs::write(&input, b"{}\n").unwrap();
    v["inputs"] =
        serde_json::json!([{"id": "t1", "role": "table", "path": input, "blake3": "0".repeat(64)}]);
    assert_eq!(pf("input.json", &v).0, 26);
    let good = blake3::hash(b"{}\n").to_hex().to_string();
    v["inputs"][0]["blake3"] = good.into();
    assert_eq!(pf("input-ok.json", &v).0, 0);
    // Lock held by a live supervisor.
    let syn = serde_json::json!({"items": 8, "case_ms": [800, 1200]});
    let lv = spec(&t, syn, serde_json::json!({}));
    let lp = write_spec(&t, "live.json", &lv);
    let mut c = start(&lp, &d, &[], &[]);
    wait_until("lock", Duration::from_secs(60), || {
        d.join("LOCK.json").exists()
    });
    let (code, out) = cli(&["preflight", "--spec", &lp.to_string_lossy(), "--dir", &ds]);
    assert_eq!(code, 25, "{out}");
    signal(&c, libc::SIGTERM);
    assert_eq!(wait(&mut c, Duration::from_secs(120)), 143);
    // A different spec under an existing campaign directory is refused by run.
    let mut c = start(&write_spec(&t, "other.json", &base), &d, &[], &[]);
    assert_eq!(wait(&mut c, Duration::from_secs(60)), 1);
    assert_no_marked(&t.campaign);
}

#[test]
fn heartbeat_timeout_kills_a_silent_child_and_memory_caps_kill() {
    let _g = serial();
    let t = Tmp::new("hb");
    let syn = serde_json::json!({"items": 8, "case_ms": [80, 200], "hang_items": [0], "p_mem": 0.4, "seed": 3});
    let spec_p = write_spec(
        &t,
        "spec.json",
        &spec(
            &t,
            syn,
            serde_json::json!({"heartbeat_timeout_s": 4, "mem_total": "130M"}),
        ),
    );
    let d = t.cdir("run");
    assert_eq!(run_to_end(&spec_p, &d, &[]), vec![finish_code_by_items(&d)]);
    let st = state(&d);
    let j = &st.jobs["N0.i0000.L0-t0"];
    assert!(
        matches!(
            j.state,
            rustred_campaign::state::AttemptState::BudgetStopped(FinishClass::KilledTime)
        ),
        "{:?}",
        j.state
    );
    assert!(matches!(
        st.items["N0.i0000.L0"].phase,
        ItemPhase::Escalated(_)
    ));
    let mem_kills = count(&d, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Finished {
                    class: FinishClass::KilledMemory,
                    ..
                },
                ..
            }
        )
    });
    assert!(mem_kills >= 1, "{}", log_tail(&d));
    let f = ledger_facts(&d);
    assert!(f.max_reserved <= 130_000_000, "{}", f.max_reserved);
    assert!(f.max_cores <= 8);
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}
