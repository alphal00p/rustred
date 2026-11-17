//! gate11 audit (WS/notes/implementer.gate11.md), finding M11-2: THE SITE POLICY NEVER GRANTS A CPU IN THE SUPERVISOR when
//! `idle_min` > 0 or `respect_foreign_pinning` is true, i.e. with RUNBOOK 1.7's site.json (0.85, true) and the letter (a)
//! host script's: the first host step as written launches nothing.
//! - `idle_min` > 0: the policy's IdleTracker is fed only by `CpuPolicy::refresh`, which the supervisor never calls
//!   (supervisor/mod.rs: no caller); `idle(c)` stays None and a CPU with no idleness is never eligible
//!   (cpu_policy.rs acquire: `None => config.idle_min <= 0.0`). Log: "admission waits: only 0 of 1 CPUs free under the
//!   site rules".
//! - `respect_foreign_pinning` true: the sampler is built with `collect_foreign_pinning: false` (supervisor/mod.rs:1056) and
//!   nothing calls `SitePolicy::set_foreign_pinned`: exclusions_of returns "foreign CPU pinning not scanned yet".
//!
//! Every supervisor-level site test of the suite sets idle_min 0.0 and respect_foreign_pinning false.
//! Both tests are ignored (they FAIL on fdec15da). Private lock directory and CPU ledger (never WS/ops). Run ONLY inside a
//! private pid namespace: `LIGHT_PIDNS=1 light_build.sh gate11 test -p rustred-campaign --test gate11_site -- --ignored`.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
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

/// A campaign under the SITE policy with a private lock directory (a lock file per CPU) and a private ledger file; returns
/// whether a job started within `limit`, and the log tail.
fn site_campaign_starts_a_job(
    tag: &str,
    idle_min: f64,
    respect_pinning: bool,
    limit: Duration,
) -> (bool, String) {
    let t = Tmp::new(tag);
    let locks = t.path.join("locks");
    std::fs::create_dir(&locks).unwrap();
    let cpus: Vec<usize> = rustred_campaign::resources::allowed_cpus()
        .unwrap()
        .into_iter()
        .filter(|c| !(168..=175).contains(c) && !(288..=319).contains(c))
        .collect();
    for c in &cpus {
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
            "candidates": null, "exclude": ["168-175", "288-319"], "held_files": [], "pause_files": [],
            "idle_min": idle_min, "max_cores": 2, "respect_foreign_pinning": respect_pinning,
            "ignore_own_tag_claims": true
        })
        .to_string(),
    )
    .unwrap();
    let mut v = spec(
        &t,
        serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "seed": 3, "case_ms": [500, 800],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 2}),
    );
    let res = v["resources"].as_object_mut().unwrap();
    res.remove("cpus");
    res.insert("policy".into(), serde_json::json!("site"));
    res.insert("site_policy".into(), serde_json::json!(site));
    let sp = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let mut c = start(&sp, &d, &["--plain"], &[]);
    let t0 = Instant::now();
    let mut started = false;
    while t0.elapsed() < limit {
        if d.join("ledger").join("MANIFEST.json").exists()
            && count(&d, |e| {
                matches!(
                    e,
                    Event::Job {
                        ev: JobEvent::Started { .. },
                        ..
                    }
                )
            }) > 0
        {
            started = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    signal(&c, libc::SIGTERM);
    let _ = wait(&mut c, Duration::from_secs(120));
    (started, log_tail(&d))
}

#[test]
#[ignore = "gate11 M11-2: FAILS on fdec15da (CpuPolicy::refresh is never called: no CPU ever has an idleness)"]
fn g11_site_policy_with_an_idle_bar_launches_a_job() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let (started, log) =
        site_campaign_starts_a_job("g11-site-idle", 0.01, false, Duration::from_secs(60));
    assert!(started, "no job started in 60 s under idle_min 0.01: {log}");
}

#[test]
#[ignore = "gate11 M11-2: FAILS on fdec15da (foreign pinning is never collected nor handed to the site policy)"]
fn g11_site_policy_respecting_foreign_pinning_launches_a_job() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let (started, log) =
        site_campaign_starts_a_job("g11-site-pin", 0.0, true, Duration::from_secs(60));
    assert!(
        started,
        "no job started in 60 s under respect_foreign_pinning: {log}"
    );
}

#[test]
fn g11_site_policy_control_without_idle_bar_or_pinning_launches_a_job() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let (started, log) =
        site_campaign_starts_a_job("g11-site-ctl", 0.0, false, Duration::from_secs(60));
    assert!(started, "control: no job started in 60 s: {log}");
}
