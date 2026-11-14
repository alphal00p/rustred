//! Lane int5, item 2 (unconfirmed launch = EXECUTION UNKNOWN, on the ledger's `unconfirmed` state; strict mode on), at
//! the supervisor level (real `rr_campaign` + `rr_fake_child`; run with LIGHT_PIDNS=1, rule L10):
//! - a live marked process of an unconfirmed launch is a LATE CONFIRMATION (`job.started` with its identity); the
//!   restart's sweep kills it and the attempt is then settled as a running attempt killed at recovery (requeued);
//! - a launch whose spawn sentinel is absent but whose run directory does NOT authenticate is never "did not run":
//!   execution unknown, item in attention, exit 8, and only the operator's `resolve` reopens the work.
//! Rule (binding): an unconfirmed launch is never "did not run" without proof and never retried automatically.

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use rustred_campaign::state::{AttemptState, ItemPhase};
use std::time::Duration;

fn job_events(d: &std::path::Path, job: &str, f: impl Fn(&JobEvent) -> bool) -> usize {
    count(d, |e| matches!(e, Event::Job { job: j, ev, .. } if j == job && f(ev)))
}

fn in_state(d: &std::path::Path, want: AttemptState) -> Vec<String> {
    state(d)
        .jobs
        .iter()
        .filter(|(_, j)| j.state == want)
        .map(|(id, _)| id.clone())
        .collect()
}

/// A crash between the spawn and `job.started` (failpoint `after_spawn`), then a process carrying exactly that
/// attempt's marker, alive in its run directory when the supervisor restarts (as an escaped helper of the child would
/// be). The restart marks the attempt `unconfirmed`, its sweep kills the marked process, records the late
/// confirmation (`started` with that pid) and settles the attempt as interrupted: requeued, the campaign equal to the
/// reference, no operator decision, no `not_spawned`.
#[test]
fn live_marked_process_of_an_unconfirmed_launch_is_a_late_confirmation() {
    let _g = serial();
    let t = Tmp::new("int5-live");
    let syn = serde_json::json!({"items": 6, "case_ms": [4000, 6000], "seed": 5, "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let r = t.cdir("reference");
    assert_eq!(run_to_end(&spec_p, &r, &[]), vec![0]);
    let reference = result(&r);

    let d = t.cdir("live");
    let mut c = start(&spec_p, &d, &[], &[("RR_CAMPAIGN_FAILPOINT", "after_spawn:2")]);
    assert_eq!(wait(&mut c, Duration::from_secs(300)), 134, "{}", log_tail(&d));
    let launching = in_state(&d, AttemptState::Launching);
    assert_eq!(launching.len(), 1, "{launching:?}");
    let job = launching[0].clone();
    let st = state(&d);
    let jr = &st.jobs[&job];
    let run = std::path::PathBuf::from(jr.run_dir.clone().unwrap());
    let id = st.inputs["campaign:marker_id"].path.clone();
    wait_until("the crashed child gone", Duration::from_secs(60), || {
        marked(&t.campaign).is_empty()
    });
    let mut planted = std::process::Command::new(sleep_bin())
        .arg("600")
        .current_dir(&run)
        .env_clear()
        .env("RR_CAMPAIGN_JOB", format!("{id}:{job}:a{}", jr.attempt))
        .spawn()
        .unwrap();
    wait_until("the planted process marked", Duration::from_secs(30), || {
        marked(&t.campaign).contains(&planted.id())
    });
    let codes = run_to_end(&spec_p, &d, &[]);
    assert_eq!(codes, vec![0], "no operator decision: {}", log_tail(&d));
    assert_eq!(result(&d), reference);
    // killed by the sweep (SIGKILL), recorded
    let status = planted.wait().unwrap();
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(libc::SIGKILL), "{status:?}");
    }
    assert!(count(&d, |e| matches!(e, Event::OrphanKilled { pid, .. } if *pid == planted.id())) >= 1);
    // unconfirmed -> started (late, the planted pid) -> interrupted / lost -> requeue
    assert_eq!(job_events(&d, &job, |e| matches!(e, JobEvent::Unconfirmed { .. })), 1);
    assert_eq!(
        job_events(&d, &job, |e| matches!(e, JobEvent::Started { pid, .. } if *pid == planted.id())),
        1
    );
    assert_eq!(job_events(&d, &job, |e| matches!(e, JobEvent::NotSpawned { .. })), 0);
    assert!(job_events(&d, &job, |e| matches!(e, JobEvent::Requeue { .. })) >= 1);
    assert!(notes(&d, "execution_unknown").is_empty());
    assert_eq!(verify(&d), 0);
    let f = ledger_facts(&d);
    assert_eq!(f.requeues, f.interrupted);
    assert_no_marked(&t.campaign);
}

/// A crash right after the launch record (failpoint `after_launch_record`: no spawn sentinel) whose run directory does
/// not authenticate any more (its `RUN/sup/CAMPAIGN` removed): the missing sentinel alone is NOT proof (the directory is
/// not provably the launched one). Execution unknown: `unconfirmed`, item in attention `execution_unknown`, exit 8,
/// never `not_spawned` / `lose` / `requeue`; the operator's `resolve` = control retry + cancel; then equal to the
/// reference.
#[test]
fn unspawned_launch_without_an_authenticated_run_directory_is_execution_unknown() {
    let _g = serial();
    let t = Tmp::new("int5-unauth");
    let syn = serde_json::json!({"items": 6, "case_ms": [1500, 2500], "seed": 5, "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let r = t.cdir("reference");
    assert_eq!(run_to_end(&spec_p, &r, &[]), vec![0]);
    let reference = result(&r);

    let d = t.cdir("unauth");
    let mut c = start(&spec_p, &d, &[], &[("RR_CAMPAIGN_FAILPOINT", "after_launch_record:2")]);
    assert_eq!(wait(&mut c, Duration::from_secs(300)), 134, "{}", log_tail(&d));
    let launching = in_state(&d, AttemptState::Launching);
    assert_eq!(launching.len(), 1, "{launching:?}");
    let job = launching[0].clone();
    let run = std::path::PathBuf::from(state(&d).jobs[&job].run_dir.clone().unwrap());
    assert!(!run.join("sup").join("SPAWN").exists(), "crash before the sentinel");
    std::fs::remove_file(run.join("sup").join("CAMPAIGN")).unwrap();

    let mut c = start(&spec_p, &d, &[], &[]);
    assert_eq!(wait(&mut c, Duration::from_secs(600)), ATTENTION, "{}", log_tail(&d));
    assert_eq!(in_state(&d, AttemptState::Unconfirmed), vec![job.clone()]);
    {
        let st = state(&d);
        let item = &st.jobs[&job].item;
        assert!(
            matches!(&st.items[item].phase, ItemPhase::Attention(c) if c == "execution_unknown"),
            "{:?}",
            st.items[item].phase
        );
    }
    assert_eq!(
        job_events(&d, &job, |e| matches!(
            e,
            JobEvent::NotSpawned { .. } | JobEvent::Lose { .. } | JobEvent::Requeue { .. } | JobEvent::Interrupt { .. }
        )),
        0,
        "never 'did not run' without proof"
    );
    assert!(
        notes(&d, "execution_unknown")
            .iter()
            .any(|n| n.contains(&job) && n.contains("not authenticated")),
        "{:?}",
        notes(&d, "execution_unknown")
    );
    // the operator decides
    let codes = run_to_end(&spec_p, &d, &[]);
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&d));
    assert_eq!(result(&d), reference);
    assert_eq!(state(&d).jobs[&job].state, AttemptState::Cancelled);
    assert_eq!(notes(&d, "control_resolve").len(), 1);
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}
