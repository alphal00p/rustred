//! Crash consistency: a writer process (this test binary re-executed with `RR_CRASH_WRITER=<cdir>`) appends
//! item lives record by record (small segments, so rotations happen too) and reports through its stdout pipe
//! `ack <item>` after each acceptance record is durable. The parent SIGKILLs it at seeded random moments, many
//! times; after each kill the ledger opens, recovers, its chain verifies, every acknowledged acceptance is in
//! the state, and no acceptance record appears twice. The writer resumes from the reduced state alone.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
mod ledger_support;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use ledger_support::*;
use rustred_campaign::clock::{Clock, SystemClock};
use rustred_campaign::ledger::{self, Event, ItemEvent, JobEvent, Ledger, LedgerConfig};
use rustred_campaign::lock;
use rustred_campaign::state::{AttemptState, CampaignState, EvidenceLevel, ItemPhase};

fn cfg() -> LedgerConfig {
    LedgerConfig {
        rotate_bytes: 32 * 1024,
        ..LedgerConfig::default()
    }
}

/// True if the effect of `e` is already in `st` (the writer's resume rule: never write a record twice).
fn already_applied(st: &CampaignState, e: &Event) -> bool {
    match e {
        Event::ItemCreated { item, .. } => st.items.contains_key(item),
        Event::JobQueued { job, .. } => st.jobs.contains_key(job),
        Event::Item { item, ev } => {
            let Some(it) = st.items.get(item) else {
                return false;
            };
            match ev {
                ItemEvent::Start { .. } => it.phase != ItemPhase::Open,
                ItemEvent::Produce { .. } => {
                    matches!(it.phase, ItemPhase::Produced | ItemPhase::Accepted(_))
                }
                ItemEvent::Accept { .. } => matches!(it.phase, ItemPhase::Accepted(_)),
                _ => false,
            }
        }
        Event::Job { job, ev, .. } => {
            let Some(j) = st.jobs.get(job) else {
                return false;
            };
            match ev {
                JobEvent::Launch { .. } => j.state != AttemptState::Queued,
                JobEvent::Started { .. } => {
                    !matches!(j.state, AttemptState::Queued | AttemptState::Launching)
                }
                JobEvent::Finished { .. } => {
                    j.state.is_settled() && j.state != AttemptState::Queued
                }
                _ => false,
            }
        }
        Event::Intent { op, .. } => st.pending_ops.contains_key(op) || st.done_ops.contains(op),
        Event::Done { op, .. } => st.done_ops.contains(op),
        _ => false,
    }
}

#[test]
fn zz_crash_writer_process() {
    let Some(dir) = std::env::var_os("RR_CRASH_WRITER") else {
        return;
    };
    let cdir = std::path::PathBuf::from(dir);
    let clk: std::sync::Arc<dyn Clock> = std::sync::Arc::new(SystemClock::new());
    // the writer process takes the campaign lock itself (a killed predecessor is taken over)
    let (lk, _) =
        lock::acquire(&cdir, &SystemClock::new(), &["writer".into()]).expect("writer locks");
    let (mut l, _) = Ledger::open(&cdir, &lk, clk, cfg()).expect("writer opens");
    let out = std::io::stdout();
    let start = l.state().items.len();
    // the last item may be half done: finish it first (resume from the reduced state)
    let first = start.saturating_sub(1);
    for k in first..first + 100_000 {
        for e in item_life(
            "T162",
            k,
            (k % 4) as u8,
            EvidenceLevel::IdentityTested,
            (k % 97) as u64 * 10 + 5,
        ) {
            if already_applied(l.state(), &e) {
                continue;
            }
            let is_accept = matches!(
                e,
                Event::Item {
                    ev: ItemEvent::Accept { .. },
                    ..
                }
            );
            l.append(e).expect("append");
            if is_accept {
                let mut o = out.lock();
                writeln!(o, "ack {}", item_id("T162", k)).unwrap();
                o.flush().unwrap();
            }
        }
    }
}

#[test]
fn sigkill_at_random_points_loses_no_acknowledged_acceptance() {
    let d = scratch("crash");
    let _c = Cleanup(d.clone());
    // the parent holds the lock only while it opens the ledger (the writer child takes it in between)
    let parent_open = |d: &std::path::Path| {
        let (lk, _) = lock::acquire(d, &SystemClock::new(), &["parent".into()]).unwrap();
        let clk = clock();
        let r = Ledger::open(d, &lk, as_dyn(&clk), cfg());
        (r, lk)
    };
    {
        let (lk, _) = lock::acquire(&d, &SystemClock::new(), &["parent".into()]).unwrap();
        let clk = clock();
        let mut l = Ledger::create(&d, &lk, as_dyn(&clk), cfg(), genesis()).unwrap();
        l.append_all(setup_events("T162")).unwrap();
    }
    let exe = std::env::current_exe().unwrap();
    let mut rng = 0x00c0_ffee_u64;
    let mut next = move || {
        rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let mut acked: BTreeSet<String> = BTreeSet::new();
    let kills = 40;
    let mut recoveries = 0;
    let mut rotations_seen = 0;
    for round in 0..kills {
        let mut child = Command::new(&exe)
            .args([
                "--exact",
                "zz_crash_writer_process",
                "--nocapture",
                "--test-threads",
                "1",
            ])
            .env("RR_CRASH_WRITER", &d)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                // the harness may print "test <name> ... " on the same line first
                if let Some(item) = line.find("ack ").map(|i| &line[i + 4..]) {
                    let _ = tx.send(item.to_string());
                }
            }
        });
        // wait for the first ack of this round (so every round makes progress), then a seeded delay
        let first = rx
            .recv_timeout(Duration::from_secs(60))
            .expect("writer acknowledges");
        acked.insert(first);
        std::thread::sleep(Duration::from_millis(1 + next() % 40));
        // SAFETY: our own child.
        unsafe { libc::kill(child.id() as i32, libc::SIGKILL) };
        let status = child.wait().unwrap();
        assert!(!status.success(), "round {round}: the writer was killed");
        reader.join().unwrap();
        acked.extend(rx.try_iter());

        // after the kill: open (recovering a torn tail if any), verify, check acknowledgements
        let (r, lk) = parent_open(&d);
        let (l, rep) = r.unwrap_or_else(|e| panic!("round {round}: open failed: {e}"));
        if rep.recovery.is_some() {
            recoveries += 1;
        }
        rotations_seen = l.manifest().sealed.len();
        let st = l.state();
        for a in &acked {
            assert!(
                matches!(
                    st.items.get(a).map(|i| &i.phase),
                    Some(ItemPhase::Accepted(_))
                ),
                "round {round}: acknowledged {a} lost"
            );
        }
        let accepted = st
            .items
            .values()
            .filter(|i| matches!(i.phase, ItemPhase::Accepted(_)))
            .count() as u64;
        assert_eq!(
            st.rules.accepted, accepted,
            "round {round}: each acceptance counted once"
        );
        drop(l);
        drop(lk);
        let chain =
            ledger::verify_chain(&d).unwrap_or_else(|e| panic!("round {round}: chain: {e}"));
        assert_eq!(chain.torn_tail_bytes, 0);
        assert_ne!(chain.snapshot_matches, Some(false));
        // no acceptance record twice in the raw ledger
        let mut count: BTreeMap<String, u32> = BTreeMap::new();
        for r in ledger::read_records(&d).unwrap() {
            if let Event::Item {
                item,
                ev: ItemEvent::Accept { .. },
            } = r.event
            {
                *count.entry(item).or_default() += 1;
            }
        }
        assert!(
            count.values().all(|&c| c == 1),
            "round {round}: duplicate acceptance records"
        );
    }
    eprintln!(
        "crash test: {kills} kills, {} acknowledged acceptances, {} accepted in total, {recoveries} torn-tail recoveries, {rotations_seen} sealed segments",
        acked.len(),
        {
            let (r, _lk) = parent_open(&d);
            r.unwrap().0.state().rules.accepted
        }
    );
    assert!(rotations_seen >= 2, "rotation exercised under kills");
}
