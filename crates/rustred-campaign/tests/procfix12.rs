//! Lane procfix7 round 8b (host step 1, WS/plan/auto-campaign/reports/HOST-STEP-1-ANALYSIS.md): a requested job scope
//! that is NOT VERIFIED is never run under "marker_only".
//!
//! The test-build failpoint `scope_pending:1` makes every job's containment `Pending` with the requested unit
//! `rr-fp-pending` ("the leader's cgroup does not name the scope"), also inside a private namespace (where the host's scope
//! mechanism is not used):
//! - a LIVE job whose scope stays unverified for 10 s is ended at once (SIGKILL) and requeued; the third such attempt of
//!   one job stops the run (exit 71, note `scope_unverified_limit`);
//! - a job that ENDS before its first check is recorded from the evidence (round 9, gate13 G13-M1): FIRST the kernel's
//!   record of the cgroup its leader exited in (inside the exact unit: `scope_acknowledged_ended`; another cgroup:
//!   `scope_unverified_ended`, whatever stderr says); only when that record is unknown, systemd-run's start
//!   acknowledgment in one of its documented forms naming the exact unit (written to the job's stderr before the exec);
//!   otherwise `scope_unverified_ended`. The failpoint `scope_exit:1` makes the kernel's record "inside the unit,
//!   removed", `scope_exit:2` "unknown"; without it the real record of the namespaced test (another cgroup) is used.
//!   The REAL systemd-run's forms are checked by tests/procfix13_real_systemd.rs (host only, one scope).
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

fn wrapper(dir: &Path, ack: Option<&str>) -> PathBuf {
    let p = dir.join("wrap.sh");
    let line = match ack {
        Some(l) => format!("echo '{l}' >&2\n"),
        None => String::new(),
    };
    std::fs::write(&p, format!("#!/bin/sh\n{line}exec {FAKE} \"$@\"\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn spec_for(t: &Tmp, ack: Option<&str>, items: u64, case_ms: [u64; 2]) -> PathBuf {
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
    let ack =
        format!("Running as unit: {UNIT}.scope; invocation ID: 0123456789abcdef0123456789abcdef");
    let sp = spec_for(&t, Some(&ack), 2, [60_000, 60_000]);
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

fn containment_whys(d: &Path) -> Vec<String> {
    notes(d, "job_containment ")
        .into_iter()
        .map(|n| {
            let j: serde_json::Value =
                serde_json::from_str(n.trim_start_matches("job_containment ")).unwrap();
            j["why"].as_str().unwrap_or("?").to_string()
        })
        .collect()
}

/// procfix7 round 9 (gate13 G13-M1): every row of the evidence table for a job that ends before its first check.
#[test]
fn b_a_job_dead_before_its_check_is_recorded_from_the_evidence() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let id = "645e842e0123456789abcdef01234567";
    let new_id = format!("Running as unit: {UNIT}.scope; invocation ID: {id}");
    let new_plain = format!("Running as unit: {UNIT}.scope");
    let old = format!("Running scope as unit: {UNIT}.scope");
    let other = format!("Running as unit: {UNIT}x.scope; invocation ID: {id}");
    // (case, stderr line, kernel failpoint, expected word, a phrase of the expected why)
    type Case<'a> = (&'a str, Option<&'a str>, Option<&'a str>, &'a str, &'a str);
    let cases: [Case; 7] = [
        (
            "kernel-inside",
            None,
            Some("scope_exit:1"),
            "scope_acknowledged_ended",
            "the kernel's record of the job's leader at its exit names it",
        ),
        (
            "ack-new-id",
            Some(&new_id),
            Some("scope_exit:2"),
            "scope_acknowledged_ended",
            "Running as unit: <unit>.scope; invocation ID: <id>",
        ),
        (
            "ack-new",
            Some(&new_plain),
            Some("scope_exit:2"),
            "scope_acknowledged_ended",
            "\"Running as unit: <unit>.scope\"",
        ),
        (
            "ack-old",
            Some(&old),
            Some("scope_exit:2"),
            "scope_acknowledged_ended",
            "Running scope as unit: <unit>.scope",
        ),
        (
            "ack-other-unit",
            Some(&other),
            Some("scope_exit:2"),
            "scope_unverified_ended",
            "holds no start acknowledgment",
        ),
        (
            "none",
            None,
            Some("scope_exit:2"),
            "scope_unverified_ended",
            "holds no start acknowledgment",
        ),
        // the namespaced test's real kernel record names another cgroup: it wins over the acknowledgment
        (
            "kernel-elsewhere",
            Some(&new_id),
            None,
            "scope_unverified_ended",
            "names another cgroup",
        ),
    ];
    for (case, line, fp, want, phrase) in cases {
        let t = Tmp::new(&format!("pf12-dead-{case}"));
        let sp = spec_for(&t, line, 3, [200, 400]);
        let d = t.cdir("c");
        let fps = match fp {
            Some(f) => format!("scope_pending:1,{f}"),
            None => "scope_pending:1".to_string(),
        };
        let mut c = start(&sp, &d, &[], &[("RR_CAMPAIGN_FAILPOINT", &fps)]);
        let code = wait(&mut c, Duration::from_secs(300));
        let words = containment_words(&d);
        let whys = containment_whys(&d);
        eprintln!(
            "PF12 dead {case}: exit {code}, containment notes {words:?}; first why {:?}",
            whys.first()
        );
        assert!(
            !words.iter().any(|w| w == "marker_only"),
            "{case}: {words:?}"
        );
        assert_eq!(words.len(), 3, "{case}: one note per attempt: {words:?}");
        assert!(words.iter().all(|w| w == want), "{case}: {words:?}");
        assert!(whys.iter().all(|w| w.contains(phrase)), "{case}: {whys:?}");
        assert_eq!(
            code, 0,
            "{case}: three short jobs, each one attempt: the run ends 0"
        );
        assert_no_marked(&t.campaign);
    }
}
