//! Lane int5, item 2 (unconfirmed launch = EXECUTION UNKNOWN, on the ledger's `unconfirmed` state; strict mode on), at
//! the supervisor level (real `rr_campaign` + `rr_fake_child`; run with LIGHT_PIDNS=1, rule L10):
//! - a live marked process of an unconfirmed launch is a LATE CONFIRMATION (`job.started` with its identity); the
//!   restart's sweep kills it and the attempt is then settled as a running attempt killed at recovery (requeued);
//! - a launch whose spawn sentinel is absent but whose run directory does NOT authenticate is never "did not run":
//!   execution unknown, item in attention, exit 8, and only the operator's `resolve` reopens the work.
//!
//! Rule (binding): an unconfirmed launch is never "did not run" without proof and never retried automatically.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use rustred_campaign::state::{AttemptState, ItemPhase};
use std::time::Duration;

fn job_events(d: &std::path::Path, job: &str, f: impl Fn(&JobEvent) -> bool) -> usize {
    count(
        d,
        |e| matches!(e, Event::Job { job: j, ev, .. } if j == job && f(ev)),
    )
}

fn in_state(d: &std::path::Path, want: AttemptState) -> Vec<String> {
    state(d)
        .jobs
        .iter()
        .filter(|(_, j)| j.state == want)
        .map(|(id, _)| id.clone())
        .collect()
}

/// What the crashed attempt's run directory holds when the supervisor restarts (procfix5 item 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Facts {
    /// As the crash left it: the child may or may not have finished. A dying supervisor that dumps core keeps its
    /// children alive (PDEATHSIG fires only at its exit): a dump of ~8 s let a 4-6 s child seal its success (the
    /// integration run of 351f1bcd, 2026-11-14). Both outcomes are legitimate; the test asserts the one the facts call for.
    AsLeft,
    /// The crashed attempt's job completed by hand before the restart (a valid sealed OUTCOME bound to the launch; for
    /// this seed the job's own result is `no_rule`, a succeeded-sealed class routed to the work source).
    SealedSuccess,
    /// As `SealedSuccess`, but the run directory does not authenticate (its `RUN/sup/CAMPAIGN` removed).
    SealedUnauthenticated,
    /// procfix5 m1: the launch went through the SPAWN BARRIER (the default since procfix5): the crashed child never ran
    /// its program (the barrier is released only after `job.started`); the planted marked process is killed and is NOT a
    /// confirmation: `not_spawned` + requeue.
    Barrier,
}

/// A crash between the spawn and `job.started` (failpoint `after_spawn`), then a process carrying exactly that
/// attempt's marker, alive when the supervisor restarts (as an escaped helper of the child would be). The restart marks
/// the attempt `unconfirmed`, its sweep kills the marked process and records the late confirmation (`started` with that
/// pid). The attempt is then settled as a running attempt killed at recovery, FROM THE FACTS ON DISK: a valid sealed
/// OUTCOME of exactly this launch in an AUTHENTICATED run directory is consumed (never re-solved), anything else is
/// interrupted and requeued (nothing read from an unauthenticated directory). In every case: no operator decision, no
/// `not_spawned`, never two attempts of the item, the campaign equal to the reference.
fn late_confirmation(facts: Facts) {
    let _g = serial();
    let t = Tmp::new("int5-live");
    let syn = serde_json::json!({"items": 6, "case_ms": [4000, 6000], "seed": 5, "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let r = t.cdir("reference");
    assert_eq!(run_to_end(&spec_p, &r, &[]), vec![0]);
    let reference = result(&r);

    let d = t.cdir("live");
    // The three item-0 fact sets concern a launch WITHOUT the spawn barrier (a ledger of an older binary; test knob
    // `no_spawn_barrier`); `Barrier` is the launch as procfix5 makes it.
    let fp = if facts == Facts::Barrier {
        "after_spawn:2"
    } else {
        "after_spawn:2,no_spawn_barrier:1"
    };
    let mut c = start(&spec_p, &d, &[], &[("RR_CAMPAIGN_FAILPOINT", fp)]);
    assert_eq!(
        wait(&mut c, Duration::from_secs(300)),
        134,
        "{}",
        log_tail(&d)
    );
    let launching = in_state(&d, AttemptState::Launching);
    assert_eq!(launching.len(), 1, "{launching:?}");
    let job = launching[0].clone();
    let st = state(&d);
    let jr = &st.jobs[&job];
    let attempt = jr.attempt;
    let run = std::path::PathBuf::from(jr.run_dir.clone().unwrap());
    let id = st.inputs["campaign:marker_id"].path.clone();
    wait_until("the crashed child gone", Duration::from_secs(60), || {
        marked(&t.campaign).is_empty()
    });
    if facts == Facts::Barrier {
        assert_eq!(
            std::fs::read_to_string(run.join("sup").join("SPAWN")).unwrap(),
            "barrier v1\n"
        );
        assert!(
            !run.join("EVENTS.jsonl").exists(),
            "the child never ran its program behind the unreleased barrier"
        );
    }
    if matches!(facts, Facts::SealedSuccess | Facts::SealedUnauthenticated)
        && !run.join("OUTCOME.json").exists()
    {
        // The crashed child's job, completed by hand (no marker: not a campaign process), as if the child had run to its
        // end: its partial files go (the fake child refuses a started run directory), job.json and RUN/sup stay.
        for f in ["EVENTS.jsonl", ".reserve"] {
            let _ = std::fs::remove_file(run.join(f));
        }
        for sub in ["out", "tmp"] {
            if let Ok(rd) = std::fs::read_dir(run.join(sub)) {
                for e in rd.flatten() {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        let s = std::process::Command::new(FAKE)
            .arg("job")
            .arg(run.join("job.json"))
            .current_dir(&run)
            .env_clear()
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(
            run.join("OUTCOME.json").exists(),
            "fake child by hand: {s:?}"
        );
    }
    if facts == Facts::SealedUnauthenticated {
        std::fs::remove_file(run.join("sup").join("CAMPAIGN")).unwrap();
    }
    // The facts the decision must follow: a sealed OUTCOME (whatever its content) in an authenticated directory.
    let outcome_left = run.join("OUTCOME.json").exists();
    let authenticated = run.join("sup").join("CAMPAIGN").exists();
    let mut planted = std::process::Command::new(sleep_bin())
        .arg("600")
        .current_dir(&run)
        .env_clear()
        .env("RR_CAMPAIGN_JOB", format!("{id}:{job}:a{attempt}"))
        .spawn()
        .unwrap();
    wait_until(
        "the planted process marked",
        Duration::from_secs(30),
        || marked(&t.campaign).contains(&planted.id()),
    );
    let codes = run_to_end(&spec_p, &d, &[]);
    assert_eq!(codes, vec![0], "no operator decision: {}", log_tail(&d));
    assert_eq!(result(&d), reference);
    // killed by the sweep (SIGKILL), recorded
    let status = planted.wait().unwrap();
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(libc::SIGKILL), "{status:?}");
    }
    assert!(
        count(
            &d,
            |e| matches!(e, Event::OrphanKilled { pid, .. } if *pid == planted.id())
        ) >= 1
    );
    // unconfirmed -> started (late, the planted pid) -> consumed OR interrupted + requeued; behind the barrier:
    // unconfirmed -> not_spawned -> requeue (never a late confirmation)
    assert_eq!(
        job_events(&d, &job, |e| matches!(e, JobEvent::Unconfirmed { .. })),
        1
    );
    let barrier = facts == Facts::Barrier;
    assert_eq!(
        job_events(
            &d,
            &job,
            |e| matches!(e, JobEvent::Started { pid, .. } if *pid == planted.id())
        ),
        usize::from(!barrier)
    );
    assert_eq!(
        job_events(&d, &job, |e| matches!(e, JobEvent::NotSpawned { .. })),
        usize::from(barrier)
    );
    let requeues = job_events(&d, &job, |e| matches!(e, JobEvent::Requeue { .. }));
    // Consumed = settled by a finish BOUND to this launch (its OUTCOME read from the run directory): any class of the
    // succeeded-sealed target (rule_found, complete, no_rule, ...), routed to the work source, never re-solved.
    let consumed = job_events(
        &d,
        &job,
        |e| matches!(e, JobEvent::Finished { binding: Some(b), .. } if b.attempt == attempt),
    );
    let unauth_interrupts = job_events(
        &d,
        &job,
        |e| matches!(e, JobEvent::Interrupt { reason } if reason.contains("not authenticated")),
    );
    match facts {
        Facts::SealedSuccess => {
            assert_eq!(
                requeues, 0,
                "a sealed OUTCOME of the launch is consumed, never re-solved"
            );
            assert_eq!(consumed, 1, "settled from the authenticated run directory");
        }
        Facts::SealedUnauthenticated => {
            assert_eq!(
                consumed, 0,
                "nothing consumed from an unauthenticated directory"
            );
            assert_eq!(unauth_interrupts, 1);
            assert!(requeues >= 1, "interrupted and requeued");
        }
        Facts::Barrier => {
            assert_eq!(consumed, 0);
            assert!(requeues >= 1);
        }
        Facts::AsLeft => {
            // the facts decide: consumed only with an OUTCOME in an authenticated directory
            if consumed == 1 {
                assert!(outcome_left && authenticated);
                assert_eq!(requeues, 0);
            } else {
                assert!(requeues >= 1);
            }
        }
    }
    // Invariants whatever the facts: exactly one of {consumed, requeued}; never two attempts of the item at once (a
    // second attempt exists only after the first was settled; no marked process of the attempt survives).
    assert_eq!(
        (consumed == 1) as u8 + (requeues >= 1) as u8,
        1,
        "consumed {consumed}, requeues {requeues}"
    );
    if consumed == 1 {
        assert_eq!(
            state(&d).jobs[&job].attempt,
            attempt,
            "no second attempt after a consumed finish"
        );
    }
    assert!(notes(&d, "execution_unknown").is_empty());
    assert_eq!(verify(&d), 0);
    let f = ledger_facts(&d);
    assert_eq!(f.requeues, f.interrupted);
    assert_no_marked(&t.campaign);
}

#[test]
fn live_marked_process_of_an_unconfirmed_launch_is_a_late_confirmation() {
    late_confirmation(Facts::AsLeft);
}

/// procfix5 m1: the same crash with the spawn barrier: the child never ran, the marked process is no confirmation.
#[test]
fn marked_process_of_an_unreleased_barrier_launch_is_not_a_confirmation() {
    late_confirmation(Facts::Barrier);
}

/// procfix5 item 0, deterministic: the crashed child sealed its success before the restart (as during a long core dump).
#[test]
fn late_confirmation_consumes_a_sealed_success_of_an_authenticated_directory() {
    late_confirmation(Facts::SealedSuccess);
}

/// procfix5 item 0: the same facts in a directory that does not authenticate: nothing is consumed, requeued.
#[test]
fn late_confirmation_never_consumes_an_unauthenticated_directory() {
    late_confirmation(Facts::SealedUnauthenticated);
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
    let mut c = start(
        &spec_p,
        &d,
        &[],
        &[("RR_CAMPAIGN_FAILPOINT", "after_launch_record:2")],
    );
    assert_eq!(
        wait(&mut c, Duration::from_secs(300)),
        134,
        "{}",
        log_tail(&d)
    );
    let launching = in_state(&d, AttemptState::Launching);
    assert_eq!(launching.len(), 1, "{launching:?}");
    let job = launching[0].clone();
    let run = std::path::PathBuf::from(state(&d).jobs[&job].run_dir.clone().unwrap());
    assert!(
        !run.join("sup").join("SPAWN").exists(),
        "crash before the sentinel"
    );
    std::fs::remove_file(run.join("sup").join("CAMPAIGN")).unwrap();

    let mut c = start(&spec_p, &d, &[], &[]);
    assert_eq!(
        wait(&mut c, Duration::from_secs(600)),
        ATTENTION,
        "{}",
        log_tail(&d)
    );
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
            JobEvent::NotSpawned { .. }
                | JobEvent::Lose { .. }
                | JobEvent::Requeue { .. }
                | JobEvent::Interrupt { .. }
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
