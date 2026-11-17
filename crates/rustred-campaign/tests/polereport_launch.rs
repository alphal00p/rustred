//! Lane tadicjob (INT8 red test, L10): the pole report's rank step runs `rr_pole_rank` as a child of the PROCESS LAYER
//! (`polereport::run_program_child` -> RealLauncher), never through a direct spawn. These tests drive that function with
//! a shell stand-in for the rank binary: the child runs in its own session under the campaign marker, with the minimal
//! contract environment (no variable of the caller leaks), stdout / stderr in a fresh run directory per attempt, its exit
//! is reported (never assumed), and the wall cap kills it through the supervisor. RUN WITH LIGHT_PIDNS=1 (spawns and
//! signals children). Test data only (L8).

use rustred_campaign::adapters::job::RunOptions;
use rustred_campaign::launch::{ExitState, KillReason};
use rustred_campaign::polereport::run_program_child;
use rustred_campaign::resources::{Sampler, SamplerConfig};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("polereport-launch-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    d
}

fn sampler() -> Arc<Sampler> {
    Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(100),
            collect_foreign_pinning: false,
        })
        .unwrap(),
    )
}

fn sh(script: &str) -> Vec<OsString> {
    vec!["-c".into(), script.into(), "rank-stand-in".into()]
}

fn opts(wall: u64) -> RunOptions {
    RunOptions {
        cpus: vec![],
        memory_cap_bytes: Some(1 << 30),
        licence_env: None,
        wall_cap: Some(Duration::from_secs(wall)),
        grace: Some(Duration::from_secs(1)),
    }
}

#[test]
fn a_rank_step_child_runs_under_the_launcher_with_the_contract_environment() {
    let d = tmp("ok");
    // a variable of the caller that must NOT reach the child (the child starts from an empty environment)
    unsafe { std::env::set_var("POLEREPORT_LAUNCH_LEAK_PROBE", "leak") };
    let s = sampler();
    let run = run_program_child(
        &s,
        std::path::Path::new("/bin/sh"),
        &sh("env > env.txt; echo '{\"rank\": 0}' > out.json; echo done; echo note >&2"),
        &d.join("runs"),
        "pole-rank-t",
        &opts(60),
    )
    .unwrap();
    assert!(run.success(), "{}", run.how());
    assert_eq!(run.report.exit, Some(ExitState::Exited(0)));
    assert!(run.report.kill_reason.is_none());
    assert!(run.report.verification.is_verified());
    assert_eq!(run.how(), "exit 0");
    assert_eq!(run.run_dir, d.join("runs").join("pole-rank-t.1"));
    // the child's cwd is its run directory; stdout / stderr are files there
    let out = std::fs::read_to_string(run.run_dir.join("out.json")).unwrap();
    assert!(out.contains("\"rank\": 0"));
    let so = std::fs::read_to_string(run.run_dir.join("stdout.log")).unwrap();
    assert_eq!(so.trim(), "done");
    assert_eq!(run.stderr_tail(), "note");
    // the contract environment: marker set by the launcher, TMPDIR = the run's private tmp, nothing of the caller's
    let env = std::fs::read_to_string(run.run_dir.join("env.txt")).unwrap();
    let marker = env
        .lines()
        .find(|l| l.starts_with("RR_CAMPAIGN_JOB="))
        .unwrap_or_else(|| panic!("no campaign marker in the child's environment:\n{env}"));
    assert!(marker.contains("pole-rank-t.1"), "{marker}");
    let tmpdir = run.run_dir.join("tmp");
    assert!(
        env.lines()
            .any(|l| l == format!("TMPDIR={}", tmpdir.display())),
        "{env}"
    );
    assert!(!env.contains("POLEREPORT_LAUNCH_LEAK_PROBE"), "{env}");
    // the label is the launcher's job label (marker attempt id)
    assert_eq!(run.report.label, "pole-rank-t.1");
}

#[test]
fn a_failing_child_is_reported_with_its_exit_and_stderr_and_attempts_never_overwrite() {
    let d = tmp("fail");
    let s = sampler();
    let runs = d.join("runs");
    let a = run_program_child(
        &s,
        std::path::Path::new("/bin/sh"),
        &sh("echo boom: no such result >&2; exit 3"),
        &runs,
        "pole-rank-f",
        &opts(60),
    )
    .unwrap();
    assert!(!a.success());
    assert_eq!(a.report.exit, Some(ExitState::Exited(3)));
    assert_eq!(a.how(), "exit 3");
    assert_eq!(a.stderr_tail(), "boom: no such result");
    // a second attempt of the same label gets the next run directory; the first one is kept as it was
    let b = run_program_child(
        &s,
        std::path::Path::new("/bin/sh"),
        &sh("echo second; exit 0"),
        &runs,
        "pole-rank-f",
        &opts(60),
    )
    .unwrap();
    assert!(b.success());
    assert_eq!(b.run_dir, runs.join("pole-rank-f.2"));
    assert_eq!(
        std::fs::read_to_string(a.run_dir.join("stderr.log"))
            .unwrap()
            .trim(),
        "boom: no such result"
    );
}

#[test]
fn the_wall_cap_ends_the_child_through_the_supervisor() {
    let d = tmp("wall");
    let s = sampler();
    let t0 = std::time::Instant::now();
    let run = run_program_child(
        &s,
        std::path::Path::new("/bin/sh"),
        &sh("sleep 120"),
        &d.join("runs"),
        "pole-rank-w",
        &opts(1),
    )
    .unwrap();
    assert!(t0.elapsed() < Duration::from_secs(60), "{:?}", t0.elapsed());
    assert!(!run.success());
    assert_eq!(run.report.kill_reason, Some(KillReason::Timeout));
    assert!(
        run.how().contains("killed by the supervisor"),
        "{}",
        run.how()
    );
    assert!(
        run.report.verification.is_verified(),
        "{:?}",
        run.report.verification
    );
}

#[test]
fn a_program_that_cannot_start_is_an_error_or_a_reported_failure_never_success() {
    let d = tmp("nostart");
    let s = sampler();
    match run_program_child(
        &s,
        &d.join("no-such-rank-binary"),
        &[],
        &d.join("runs"),
        "pole-rank-n",
        &opts(60),
    ) {
        Err(e) => assert!(e.contains("launch of"), "{e}"),
        Ok(run) => assert!(!run.success(), "{}", run.how()),
    }
}
