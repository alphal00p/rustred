//! I2-I6 of plan 6.5 as far as they concern the supervisor with the fake child: kills at failpoints (during
//! `launching`, after the spawn, between table seal and ledger acceptance, after it, during a progress write), a full
//! disk on the ledger, children that ignore SIGTERM, a child whose helper escapes the group, a second supervisor (refused),
//! a stale lock after SIGKILL with DECOYS (a process without the marker whose identity is recorded as an attempt's, a
//! process with another campaign key's marker), and a changed binary between stop and resume (refused, then stale
//! marking with --accept-binary-change). Every run ends equal to its reference (accepted set, dispositions, terminals).

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent, Ledger, LedgerConfig};
use rustred_campaign::state::{FinishClass, Rung};
use std::sync::Arc;
use std::time::Duration;

fn small() -> serde_json::Value {
    serde_json::json!({"items": 16, "case_ms": [80, 200], "seed": 5})
}

fn reference(
    t: &Tmp,
    spec_p: &std::path::Path,
) -> rustred_campaign::supervisor::result::FinalResult {
    let d = t.cdir("reference");
    assert_eq!(run_to_end(spec_p, &d, &[]), vec![finish_code_by_items(&d)]);
    result(&d)
}

#[test]
fn failpoints_and_full_disk_resume_to_the_reference() {
    let _g = serial();
    let t = Tmp::new("fp");
    let spec_p = write_spec(&t, "spec.json", &spec(&t, small(), serde_json::json!({})));
    let reference = reference(&t, &spec_p);
    for (i, (fp, want)) in [
        ("after_launch_record:3", 134),
        ("after_spawn:5", 134),
        ("before_settle_commit:4", 134),
        ("after_settle_commit:6", 134),
        ("progress_write:3", 134),
        ("ledger_enospc:40", 75),
    ]
    .iter()
    .enumerate()
    {
        let d = t.cdir(&format!("fp{i}"));
        let mut c = start(&spec_p, &d, &[], &[("RR_CAMPAIGN_FAILPOINT", fp)]);
        let code = wait(&mut c, Duration::from_secs(300));
        assert_eq!(code, *want, "{fp}: {}", log_tail(&d));
        run_to_end(&spec_p, &d, &[]);
        assert_eq!(result(&d), reference, "{fp}");
        let f = ledger_facts(&d);
        assert_eq!(f.requeues, f.interrupted, "{fp}");
        assert_eq!(verify(&d), 0, "{fp}");
        assert_no_marked(&t.campaign);
        if fp.starts_with("before_settle") {
            // The sealed success of the killed session was accepted after the restart (idempotent acceptance).
            assert!(
                count(&d, |e| matches!(
                    e,
                    Event::Item {
                        ev: rustred_campaign::ledger::ItemEvent::Accept { .. },
                        ..
                    }
                )) > 0
            );
        }
        if fp.starts_with("ledger_enospc") {
            let full = std::fs::read_to_string(d.with_extension("log")).unwrap();
            assert!(full.contains("disk full"), "{}", log_tail(&d));
        }
    }
}

#[test]
fn children_ignoring_sigterm_are_killed_after_the_grace() {
    let _g = serial();
    let t = Tmp::new("ign");
    // Cases far longer than grace + 5 s, so that the drain must end in the group kill.
    let syn = serde_json::json!({"items": 8, "case_ms": [9000, 12000], "seed": 5, "on_sigterm": "ignore",
        "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(
        &t,
        "spec.json",
        &spec(&t, syn, serde_json::json!({"grace_seconds": 1})),
    );
    let reference = reference(&t, &spec_p);
    let d = t.cdir("run");
    let mut c = start(&spec_p, &d, &[], &[]);
    wait_until("a running job", Duration::from_secs(120), || {
        ledger_ready(&d)
            && count(&d, |e| {
                matches!(
                    e,
                    Event::Job {
                        ev: JobEvent::Started { .. },
                        ..
                    }
                )
            }) >= 2
    });
    let t0 = std::time::Instant::now();
    signal(&c, libc::SIGINT);
    assert_eq!(wait(&mut c, Duration::from_secs(120)), 130);
    // grace 1 s + 5 s, then the group kill and the empty-session check.
    assert!(t0.elapsed() < Duration::from_secs(60), "{:?}", t0.elapsed());
    let killed = count(&d, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Finished {
                    class: FinishClass::Interrupted,
                    signal: Some(9),
                    ..
                },
                ..
            }
        )
    });
    assert!(killed >= 1, "{}", log_tail(&d));
    assert_no_marked(&t.campaign);
    run_to_end(&spec_p, &d, &[]);
    assert_eq!(result(&d), reference);
    assert_eq!(verify(&d), 0);
}

#[test]
fn helpers_that_escape_the_group_are_killed_by_marker() {
    let _g = serial();
    let t = Tmp::new("esc");
    let syn =
        serde_json::json!({"items": 6, "case_ms": [80, 200], "seed": 5, "escape_helpers": true});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let d = t.cdir("run");
    assert_eq!(run_to_end(&spec_p, &d, &[]), vec![finish_code_by_items(&d)]);
    // Ledger kind `orphan_killed` (job recorded), and the log names the contract violation.
    let killed = count(&d, |e| {
        matches!(e, Event::OrphanKilled { job: Some(_), .. })
    });
    assert!(killed >= 1, "{}", log_tail(&d));
    assert!(notes(&d, "orphan_killed").is_empty());
    let log = std::fs::read_to_string(d.join("log/supervisor.log")).unwrap();
    assert!(log.contains("contract violation"), "{log}");
    assert_no_marked(&t.campaign);
    assert_eq!(verify(&d), 0);
}

#[test]
fn second_supervisor_refused_stale_lock_taken_over_decoys_survive() {
    let _g = serial();
    let t = Tmp::new("lock");
    let syn = serde_json::json!({"items": 16, "case_ms": [600, 1200], "seed": 5});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let reference = reference(&t, &spec_p);
    let d = t.cdir("run");
    let mut a = start(&spec_p, &d, &[], &[]);
    wait_until("first launches", Duration::from_secs(120), || {
        ledger_ready(&d)
            && count(&d, |e| {
                matches!(
                    e,
                    Event::Job {
                        ev: JobEvent::Started { .. },
                        ..
                    }
                )
            }) >= 2
    });
    // A second supervisor on the same directory is refused while the first runs.
    let mut b = start(&spec_p, &d, &[], &[]);
    assert_eq!(wait(&mut b, Duration::from_secs(60)), 1);
    assert!(log_tail(&d).contains("live supervisor"), "{}", log_tail(&d));
    // SIGKILL of the first: a stale lock, children die by PDEATHSIG.
    signal(&a, libc::SIGKILL);
    assert_eq!(wait(&mut a, Duration::from_secs(30)), 137);
    // Decoys: no marker but its identity recorded as an attempt's (pid, start ticks, boot id, session number = pid),
    // cwd inside the campaign directory; and a process carrying ANOTHER campaign key's marker (same campaign id).
    let d1 = decoy(&d, None);
    let other = format!(
        "RR_CAMPAIGN_JOB={}.ffffffffffffffff:N0.i0000.L0-t0:a1",
        t.campaign
    );
    let (ok, ov) = other.split_once('=').unwrap();
    let d2 = decoy(&d, Some((ok, ov)));
    std::thread::sleep(Duration::from_millis(200));
    {
        // Planting into the DEAD supervisor's ledger needs a write handle, i.e. the campaign lock (ledger 4): take it
        // over, plant, release, and put the dead owner's record back, so the restart still meets a stale lock.
        let owner_file = d.join(rustred_campaign::lock::OWNER_FILE);
        let dead_owner = std::fs::read(&owner_file).unwrap();
        let clock = Arc::new(rustred_campaign::clock::SystemClock::new());
        let (lock, _) =
            rustred_campaign::lock::acquire(&d, clock.as_ref(), &["decoy-planter".into()]).unwrap();
        let (mut l, _) = Ledger::open(&d, &lock, clock, LedgerConfig::default()).unwrap();
        let run_dir = t.path.join("decoy-run");
        std::fs::create_dir_all(&run_dir).unwrap();
        l.append_all(vec![
            Event::NodeRegistered {
                node: "DECOY".into(),
                inputs: vec![],
                terminal_bound: None,
                spec_box: None,
                spec: Default::default(),
                reference: None,
            },
            Event::ItemCreated {
                item: "DECOY.i0".into(),
                node: "DECOY".into(),
                kind: "decoy".into(),
                rung: Rung(0),
                parent: None,
                inputs: vec![],
                label: "decoy".into(),
                weight: 1,
            },
            Event::JobQueued {
                job: "DECOY.i0-t0".into(),
                item: "DECOY.i0".into(),
                rung: Rung(0),
                kind: "fake".into(),
                spec_blake3: "0".repeat(64),
                binary: "bin:rr_fake_child".into(),
                reserved_bytes: 1,
                inputs: vec![],
            },
            Event::Job {
                job: "DECOY.i0-t0".into(),
                attempt: 1,
                ev: JobEvent::Launch {
                    run_dir: run_dir.to_string_lossy().into(),
                    cpus: vec![],
                    reserved_bytes: 1,
                    spec_blake3: "0".repeat(64),
                    scope_unit: None,
                },
            },
            Event::Job {
                job: "DECOY.i0-t0".into(),
                attempt: 1,
                ev: JobEvent::Started {
                    pid: d1.id(),
                    start_ticks: start_ticks(d1.id()),
                    boot_id: rustred_campaign::resources::boot_id().unwrap(),
                },
            },
        ])
        .unwrap();
        drop(l);
        lock.release().unwrap();
        std::fs::write(&owner_file, dead_owner).unwrap();
    }
    // Immediate restart: the stale lock is taken over; recovery settles everything; the run finishes.
    run_to_end(&spec_p, &d, &[]);
    assert!(count(&d, |e| matches!(e, Event::LockTakeover { .. })) >= 1);
    let mut d1 = d1;
    let mut d2 = d2;
    assert!(
        d1.try_wait().unwrap().is_none(),
        "decoy without marker was signalled"
    );
    assert!(
        d2.try_wait().unwrap().is_none(),
        "decoy of another campaign key was signalled"
    );
    let r = result(&d);
    assert_eq!(r.accepted, reference.accepted);
    assert_eq!(r.terminals, reference.terminals);
    assert_eq!(verify(&d), 0);
    assert_no_marked_except(&t.campaign, &[d2.id()]);
    let _ = d1.kill();
    let _ = d2.kill();
    let _ = d1.wait();
    let _ = d2.wait();
}

#[test]
fn changed_binary_is_refused_then_marks_rows_stale() {
    let _g = serial();
    let t = Tmp::new("bin");
    let copy = t.path.join("rr_fake_child.copy");
    std::fs::copy(FAKE, &copy).unwrap();
    let mut v = spec(&t, small(), serde_json::json!({}));
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(copy);
    let spec_p = write_spec(&t, "spec.json", &v);
    let reference = reference(&t, &spec_p);
    let d = t.cdir("run");
    let mut c = start(&spec_p, &d, &[], &[]);
    wait_until("some accepted rules", Duration::from_secs(120), || {
        ledger_ready(&d)
            && count(&d, |e| {
                matches!(
                    e,
                    Event::Item {
                        ev: rustred_campaign::ledger::ItemEvent::Accept { .. },
                        ..
                    }
                )
            }) >= 2
    });
    signal(&c, libc::SIGINT);
    assert_eq!(wait(&mut c, Duration::from_secs(120)), 130);
    // Change the binary (an appended byte keeps the ELF runnable; the digest differs).
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&copy)
        .unwrap()
        .write_all(b"\0")
        .unwrap();
    let mut c = start(&spec_p, &d, &[], &[]);
    assert_eq!(wait(&mut c, Duration::from_secs(60)), 6);
    assert!(log_tail(&d).contains("binary changed"), "{}", log_tail(&d));
    run_to_end(&spec_p, &d, &["--accept-binary-change"]);
    let st = state(&d);
    assert_eq!(st.inputs["bin:rr_fake_child"].versions, 2);
    assert!(
        count(&d, |e| matches!(
            e,
            Event::Item {
                ev: rustred_campaign::ledger::ItemEvent::Reopen { .. },
                ..
            }
        )) >= 2
    );
    let r = result(&d);
    assert_eq!(r, reference);
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}
