//! procfix3 regressions at the supervisor level (real `rr_campaign` + `rr_fake_child`, LIGHT_PIDNS=1, rule L10):
//! item 6 (unconfirmed launch = execution unknown), item 4 (supervisor resource errors at launch), gate-B m1 (control
//! order), gate-B m2 (verify reasons).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use rustred_campaign::state::AttemptState;
use std::time::Duration;

/// Jobs whose attempt is in `state`.
fn in_state(d: &std::path::Path, want: AttemptState) -> Vec<String> {
    state(d)
        .jobs
        .iter()
        .filter(|(_, j)| j.state == want)
        .map(|(id, _)| id.clone())
        .collect()
}

/// Jobs whose attempt is still `launching` (journalled, never confirmed; before the restart).
fn launching(d: &std::path::Path) -> Vec<String> {
    in_state(d, AttemptState::Launching)
}

/// Jobs whose attempt is `unconfirmed` (lane int5: marked by the restart's recovery hook; execution unknown).
fn unconfirmed(d: &std::path::Path) -> Vec<String> {
    in_state(d, AttemptState::Unconfirmed)
}

/// Ledger records `job {job, ev}` matching `f`.
fn job_events(d: &std::path::Path, job: &str, f: impl Fn(&JobEvent) -> bool) -> usize {
    count(
        d,
        |e| matches!(e, Event::Job { job: j, ev, .. } if j == job && f(ev)),
    )
}

/// Item 6 (requirement 05:23Z), part 1, Astra's hazard: a launch journalled but never confirmed (crash between the
/// spawn and `job.started`) whose job IN FACT RAN and sealed valid results (here: the attempt's own job.json run to
/// completion by hand after the crash). The restart AUTHENTICATES the run directory and CONSUMES the valid OUTCOME:
/// no re-solve, no requeue of that attempt, no execution-unknown stop. A crash BEFORE the spawn sentinel
/// (after_launch_record) is proven "never spawned" and requeued automatically.
#[test]
fn unconfirmed_launch_with_a_valid_outcome_is_consumed_and_unspawned_is_requeued() {
    let _g = serial();
    let t = Tmp::new("unk1");
    let syn = serde_json::json!({"items": 6, "case_ms": [4000, 6000], "seed": 5, "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let r = t.cdir("reference");
    assert_eq!(run_to_end(&spec_p, &r, &[]), vec![finish_code_by_items(&r)]);
    let reference = result(&r);

    let d = t.cdir("spawned");
    let mut c = start(
        &spec_p,
        &d,
        &[],
        // procfix5: the int5 / procfix3 rules for a launch WITHOUT the spawn barrier (older binaries; test knob)
        &[("RR_CAMPAIGN_FAILPOINT", "after_spawn:2,no_spawn_barrier:1")],
    );
    assert_eq!(
        wait(&mut c, Duration::from_secs(300)),
        134,
        "{}",
        log_tail(&d)
    );
    let unconfirmed = launching(&d);
    assert_eq!(unconfirmed.len(), 1, "{unconfirmed:?}");
    let run = std::path::PathBuf::from(
        state(&d).jobs[&unconfirmed[0]]
            .run_dir
            .clone()
            .expect("launch record names the run directory"),
    );
    // The killed child is gone (PDEATHSIG); its job then "runs" to completion (as if it had been unaffected).
    wait_until("the crashed child gone", Duration::from_secs(60), || {
        marked(&t.campaign).is_empty()
    });
    {
        use rustred_campaign_contract::layout::{EVENTS, OUTCOME, RESERVE, RESUME};
        for f in [OUTCOME, EVENTS, RESUME, RESERVE] {
            let _ = std::fs::remove_file(run.join(f));
        }
    }
    let _ = std::fs::remove_dir_all(run.join("out"));
    std::fs::create_dir_all(run.join("out")).unwrap();
    let status = std::process::Command::new(FAKE)
        .arg("job")
        .arg(run.join("job.json"))
        .current_dir(&run)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", &run)
        .env("LANG", "C")
        .env("TMPDIR", run.join("tmp"))
        .output()
        .unwrap();
    assert!(
        run.join("OUTCOME.json").exists(),
        "fake child: {:?} {}",
        status.status,
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(
        run_to_end(&spec_p, &d, &[]),
        vec![finish_code_by_items(&d)],
        "{}",
        log_tail(&d)
    );
    assert_eq!(result(&d), reference);
    assert!(notes(&d, "execution_unknown").is_empty());
    let j = &state(&d).jobs[&unconfirmed[0]];
    assert_eq!(j.attempt, 1, "consumed, not requeued: {j:?}");
    assert!(j.state.is_settled());
    // lane int5: marked `unconfirmed` by the recovery hook, then settled by a `finished` BOUND to exactly this launch.
    assert_eq!(
        job_events(&d, &unconfirmed[0], |e| matches!(
            e,
            JobEvent::Unconfirmed { .. }
        )),
        1
    );
    let launched = j.attempt_spec.clone().unwrap();
    assert_eq!(
        job_events(&d, &unconfirmed[0], |e| matches!(
            e,
            JobEvent::Finished { binding: Some(b), outcome: Some(_), .. }
                if b.attempt == 1 && b.spec_blake3 == launched
        )),
        1
    );
    assert_eq!(verify(&d), 0);

    let d = t.cdir("never-spawned");
    let mut c = start(
        &spec_p,
        &d,
        &[],
        &[("RR_CAMPAIGN_FAILPOINT", "after_launch_record:2")],
    );
    assert_eq!(wait(&mut c, Duration::from_secs(300)), 134);
    assert_eq!(
        run_to_end(&spec_p, &d, &[]),
        vec![finish_code_by_items(&d)],
        "{}",
        log_tail(&d)
    );
    assert_eq!(result(&d), reference);
    assert!(notes(&d, "execution_unknown").is_empty());
    // lane int5: the only automatic requeue of an unconfirmed launch is the PROVEN one (`not_spawned`); never `lose`.
    assert!(
        count(&d, |e| matches!(
            e,
            Event::Job {
                ev: JobEvent::NotSpawned { reason },
                ..
            } if reason.contains("SPAWN")
        )) >= 1
    );
    assert!(
        count(&d, |e| matches!(
            e,
            Event::Job {
                ev: JobEvent::Unconfirmed { .. },
                ..
            }
        )) >= 1
    );
    let f = ledger_facts(&d);
    assert_eq!(f.requeues, f.interrupted);
    assert_no_marked(&t.campaign);
}

/// Item 6, part 2: a crash between the spawn and `job.started` whose child was killed before it could write OUTCOME
/// (long cases): EXECUTION UNKNOWN. The restart marks it `unconfirmed`, puts its item in `attention
/// {execution_unknown}` and never requeues it by itself (exit 8 once nothing else is left, a ledger note, again at the
/// next start); a `resolve` of another job is refused; `control resolve JOB` = `control retry` of the item + `cancel`
/// of exactly that attempt (lane int5), and the campaign ends equal to the reference.
#[test]
fn unconfirmed_launch_without_outcome_waits_for_the_operator() {
    let _g = serial();
    let t = Tmp::new("unk2");
    let syn = serde_json::json!({"items": 6, "case_ms": [4000, 6000], "seed": 5, "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let r = t.cdir("reference");
    assert_eq!(run_to_end(&spec_p, &r, &[]), vec![finish_code_by_items(&r)]);
    let reference = result(&r);
    let d = t.cdir("spawned");
    let mut c = start(
        &spec_p,
        &d,
        &[],
        // procfix5: the int5 / procfix3 rules for a launch WITHOUT the spawn barrier (older binaries; test knob)
        &[("RR_CAMPAIGN_FAILPOINT", "after_spawn:2,no_spawn_barrier:1")],
    );
    assert_eq!(
        wait(&mut c, Duration::from_secs(300)),
        134,
        "{}",
        log_tail(&d)
    );
    let unknown = launching(&d);
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    let mut c = start(&spec_p, &d, &[], &[]);
    assert_eq!(
        wait(&mut c, Duration::from_secs(600)),
        8,
        "{}",
        log_tail(&d)
    );
    assert_eq!(unconfirmed(&d), unknown, "never requeued automatically");
    assert!(launching(&d).is_empty());
    {
        let st = state(&d);
        let item = &st.jobs[&unknown[0]].item;
        assert!(
            matches!(&st.items[item].phase, rustred_campaign::state::ItemPhase::Attention(c) if c == "execution_unknown"),
            "{:?}",
            st.items[item].phase
        );
    }
    assert_eq!(
        job_events(&d, &unknown[0], |e| matches!(
            e,
            JobEvent::Lose { .. }
                | JobEvent::Interrupt { .. }
                | JobEvent::Requeue { .. }
                | JobEvent::NotSpawned { .. }
        )),
        0,
        "an unconfirmed launch is never 'did not run'"
    );
    assert!(
        notes(&d, "execution_unknown")
            .iter()
            .any(|n| n.contains(&unknown[0])),
        "{:?}",
        notes(&d, "execution_unknown")
    );
    let mut c = start(&spec_p, &d, &[], &[]);
    assert_eq!(wait(&mut c, Duration::from_secs(600)), 8);
    assert_eq!(unconfirmed(&d), unknown);
    assert_eq!(
        cli(&["control", d.to_str().unwrap(), "resolve", "N9.nothing-t0"]).0,
        0
    );
    let (code, _) = cli(&[
        "control",
        d.to_str().unwrap(),
        "resolve",
        &unknown[0],
        "--reason",
        "checked by hand",
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        run_to_end(&spec_p, &d, &[]),
        vec![finish_code_by_items(&d)],
        "{}",
        log_tail(&d)
    );
    assert_eq!(result(&d), reference);
    assert_eq!(notes(&d, "control_resolve").len(), 1);
    assert!(
        notes(&d, "control_refused")
            .iter()
            .any(|n| n.contains("N9.nothing-t0"))
    );
    // the resolved attempt: cancelled (never lost / requeued); its item's work ran again as a NEW job
    assert_eq!(state(&d).jobs[&unknown[0]].state, AttemptState::Cancelled);
    assert_eq!(
        job_events(&d, &unknown[0], |e| matches!(e, JobEvent::Cancel { .. })),
        1
    );
    assert_eq!(
        job_events(&d, &unknown[0], |e| matches!(
            e,
            JobEvent::Lose { .. } | JobEvent::Requeue { .. }
        )),
        0
    );
    assert_eq!(verify(&d), 0);
    let f = ledger_facts(&d);
    assert_eq!(f.requeues, f.interrupted);
    assert_no_marked(&t.campaign);
}

/// gate-B m1: requests of the same second keep the order in which they were written (sequence numbers).
#[test]
fn control_requests_of_one_second_apply_in_written_order() {
    let _g = serial();
    let t = Tmp::new("ord");
    let syn = serde_json::json!({"items": 6, "case_ms": [1500, 2500], "seed": 5});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let d = t.cdir("run");
    let mut c = start(&spec_p, &d, &[], &[]);
    wait_until("ledger", Duration::from_secs(120), || {
        d.join("ledger").join("MANIFEST.json").exists()
    });
    // pause then resume, written within one second, many times: the last one (resume) must win every time
    for _ in 0..5 {
        assert_eq!(cli(&["control", d.to_str().unwrap(), "pause"]).0, 0);
        assert_eq!(cli(&["control", d.to_str().unwrap(), "resume"]).0, 0);
    }
    let code = wait(&mut c, Duration::from_secs(600));
    // procfix5 (gate4 m3/m4): the exit code follows the items (10 when a synthetic item crashed twice)
    assert_eq!(code, finish_code_by_items(&d), "{}", log_tail(&d));
    let ctl = controls(&d);
    assert_eq!(ctl.len(), 10, "{ctl:?}");
    assert!(!state(&d).paused);
}

/// The campaign's marker id as recorded in the ledger (input `campaign:marker_id`).
fn marker_id(d: &std::path::Path) -> String {
    live_state(d).inputs["campaign:marker_id"].path.clone()
}

/// The state of a LIVE campaign (read-only open only; the harness's `state` cross-checks two reads).
fn live_state(d: &std::path::Path) -> rustred_campaign::state::CampaignState {
    rustred_campaign::ledger::open_read_only(d).unwrap().state
}

/// gate-B M1: marked re-fork chains that start a NEW SESSION at every hop (and plain ones), orphaned by a supervisor
/// CRASH (SIGKILL), are not in the restarted supervisor's process tree. The restart's freezing sweep (and the periodic
/// and final sweeps) stop and kill every one through the guard; at FINISHED no process carries the campaign's marker.
/// Lane int5 (gate3 M1, directing session's ruling): a marked process whose cwd is OUTSIDE the campaign directories
/// is ours by the marker rule too: killed through the guard and recorded (`orphan_killed`, `signal_sent`), no
/// `marked_survivor` (the former cwd rule left it alive forever).
#[test]
fn marked_refork_chains_orphaned_by_a_crash_do_not_survive_the_restart() {
    let _g = serial();
    let t = Tmp::new("chain");
    let syn = serde_json::json!({"items": 10, "case_ms": [1500, 2500], "seed": 7, "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let d = t.cdir("run");
    let relay = t.path.join("relay.sh");
    std::fs::write(
        &relay,
        "#!/bin/sh\n# relay MODE DEADLINE_EPOCH: each hop forks the next (setsid or not) and exits\nsleep 0.2\nnow=$(date +%s)\n[ \"$now\" -ge \"$2\" ] && exit 0\nif [ \"$1\" = setsid ]; then setsid \"$0\" \"$@\" & else \"$0\" \"$@\" & fi\nexit 0\n",
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&relay, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut c = start(&spec_p, &d, &[], &[]);
    wait_until("a started job", Duration::from_secs(120), || {
        d.join("ledger").join("MANIFEST.json").exists()
            && count(&d, |e| {
                matches!(
                    e,
                    Event::Job {
                        ev: JobEvent::Started { .. },
                        ..
                    }
                )
            }) >= 1
    });
    let id = marker_id(&d);
    let (job, run_dir) = live_state(&d)
        .jobs
        .iter()
        .find_map(|(j, r)| r.run_dir.clone().map(|rd| (j.clone(), rd)))
        .unwrap();
    let deadline = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 600;
    let mut starters = Vec::new();
    for (k, mode) in ["setsid", "setsid", "setsid", "setsid", "plain", "plain"]
        .iter()
        .enumerate()
    {
        starters.push(
            std::process::Command::new(&relay)
                .args([*mode, &deadline.to_string()])
                .current_dir(&run_dir)
                .env_clear()
                .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                .env("RR_CAMPAIGN_JOB", format!("{id}:{job}:a{}", k + 1))
                .spawn()
                .unwrap(),
        );
    }
    // A marked process OUTSIDE the campaign directories (cwd /): reported, never signalled.
    let mut outside = std::process::Command::new(sleep_bin())
        .arg("600")
        .current_dir("/")
        .env_clear()
        .env("RR_CAMPAIGN_JOB", format!("{id}:{job}:a99"))
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_secs(2));
    assert!(
        marked(&t.campaign).len() >= 7,
        "chains running: {:?}",
        marked(&t.campaign)
    );
    signal(&c, libc::SIGKILL);
    assert_eq!(wait(&mut c, Duration::from_secs(60)), 137);
    std::thread::sleep(Duration::from_secs(1));
    assert!(
        marked(&t.campaign).len() >= 7,
        "chains survive the crash: {:?}",
        marked(&t.campaign)
    );
    let codes = run_to_end(&spec_p, &d, &[]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d));
    let left: Vec<u32> = marked(&t.campaign);
    assert!(
        left.is_empty(),
        "marked processes alive after FINISHED: {left:?}\n{}",
        log_tail(&d)
    );
    // the cwd-/ process: killed (SIGKILL through the guard) and recorded, never reported as a survivor
    let st = outside.wait().unwrap();
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(st.signal(), Some(libc::SIGKILL), "{st:?}");
    }
    let opid = outside.id();
    assert!(
        count(
            &d,
            |e| matches!(e, Event::OrphanKilled { pid, .. } if *pid == opid)
        ) >= 1,
        "the cwd-/ kill is recorded"
    );
    assert!(
        count(
            &d,
            |e| matches!(e, Event::SignalSent { pid, signal, .. } if *pid == opid && *signal == libc::SIGKILL)
        ) >= 1,
        "its SIGKILL is a signal_sent record"
    );
    assert!(
        notes(&d, "marked_survivor").is_empty(),
        "{:?}",
        notes(&d, "marked_survivor")
    );
    assert!(
        count(&d, |e| matches!(e, Event::OrphanKilled { .. })) >= 6,
        "every chain member killed is recorded"
    );
    for mut s in starters {
        let _ = s.wait();
    }
    assert_no_marked(&t.campaign);
}
