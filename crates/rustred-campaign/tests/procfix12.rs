//! Lane procfix7 round 8b (host step 1, WS/plan/auto-campaign/reports/HOST-STEP-1-ANALYSIS.md): a requested job scope
//! that is NOT VERIFIED is never run under "marker_only".
//!
//! The test-build failpoint `scope_pending:1` makes every job's containment `Pending` with the requested unit
//! `rr-fp-pending` ("the leader's cgroup does not name the scope"), also inside a private namespace (where the host's scope
//! mechanism is not used):
//! - a LIVE job whose scope stays unverified for 10 s is ended at once (SIGKILL) and requeued; the third such attempt of
//!   one job stops the run (exit 71, note `scope_unverified_limit`);
//! - a job that ENDS before its first check is recorded from the evidence: systemd-run's start acknowledgment
//!   ("Running scope as unit: <unit>.scope", written to the job's stderr before the exec) gives
//!   `scope_acknowledged_ended`; without it `scope_unverified_ended`;
//! - never a `job_containment` note "marker_only".
//!
//! Spawns processes and sends signals: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

const UNIT: &str = "rr-fp-pending";

fn in_ns() -> bool {
    if rustred_campaign::launch::in_private_namespace() {
        return true;
    }
    eprintln!(
        "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
    );
    false
}

fn wrapper(dir: &Path, ack: bool) -> PathBuf {
    let p = dir.join("wrap.sh");
    let line = if ack {
        format!("echo 'Running scope as unit: {UNIT}.scope' >&2\n")
    } else {
        String::new()
    };
    std::fs::write(&p, format!("#!/bin/sh\n{line}exec {FAKE} \"$@\"\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn spec_for(t: &Tmp, ack: bool, items: u64, case_ms: [u64; 2]) -> PathBuf {
    let wrap = wrapper(&t.path, ack);
    let mut v = spec(
        t,
        serde_json::json!({"items": items, "nodes": 1, "rungs": 1, "seed": 3, "case_ms": case_ms,
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 1}),
    );
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    write_spec(t, "spec.json", &v)
}

fn containment_words(d: &Path) -> Vec<String> {
    notes(d, "job_containment ")
        .into_iter()
        .map(|n| {
            let j: serde_json::Value =
                serde_json::from_str(n.trim_start_matches("job_containment ")).unwrap();
            j["containment"].as_str().unwrap_or("?").to_string()
        })
        .collect()
}

#[test]
fn b_a_live_job_without_a_verified_scope_is_ended_requeued_and_bounded() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf12-live");
    let sp = spec_for(&t, true, 2, [60_000, 60_000]);
    let d = t.cdir("c");
    let mut c = start(
        &sp,
        &d,
        &[],
        &[("RR_CAMPAIGN_FAILPOINT", "scope_pending:1")],
    );
    let code = wait(&mut c, Duration::from_secs(600));
    let words = containment_words(&d);
    eprintln!(
        "PF12 live: exit {code}, containment notes {words:?}; {}",
        log_tail(&d)
            .lines()
            .filter(|l| l.contains("containment") || l.contains("scope"))
            .take(8)
            .collect::<Vec<_>>()
            .join(" | ")
    );
    kill_marked(&t.campaign);
    assert_eq!(
        code, 71,
        "the third unverified attempt of one job stops the run (exit 71)"
    );
    assert!(!words.iter().any(|w| w == "marker_only"), "{words:?}");
    assert!(
        words
            .iter()
            .filter(|w| *w == "scope_unverified_ended")
            .count()
            >= 3,
        "{words:?}"
    );
    let lim = notes(&d, "scope_unverified_limit ");
    assert_eq!(lim.len(), 1, "{lim:?}");
    // every ended attempt was requeued: attempts 1..3 of the same job, each ended long before its 60 s case
    let st = state(&d);
    assert!(
        st.jobs.values().any(|j| j.attempt >= 3),
        "a job reached its third attempt"
    );
}

#[test]
fn b_a_job_dead_before_its_check_is_recorded_from_the_start_acknowledgment() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    for ack in [true, false] {
        let t = Tmp::new(&format!("pf12-dead-{ack}"));
        let sp = spec_for(&t, ack, 3, [200, 400]);
        let d = t.cdir("c");
        let mut c = start(
            &sp,
            &d,
            &[],
            &[("RR_CAMPAIGN_FAILPOINT", "scope_pending:1")],
        );
        let code = wait(&mut c, Duration::from_secs(300));
        let words = containment_words(&d);
        eprintln!("PF12 dead ack={ack}: exit {code}, containment notes {words:?}");
        assert!(!words.iter().any(|w| w == "marker_only"), "{words:?}");
        assert_eq!(words.len(), 3, "one note per attempt: {words:?}");
        let want = if ack {
            "scope_acknowledged_ended"
        } else {
            "scope_unverified_ended"
        };
        assert!(words.iter().all(|w| w == want), "ack {ack}: {words:?}");
        assert_eq!(
            code, 0,
            "three short jobs, each one attempt: the run ends 0"
        );
        assert_no_marked(&t.campaign);
    }
}
