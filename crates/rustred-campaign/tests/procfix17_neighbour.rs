//! Lane procfix7 round 17 (host run B from WS/bin/frozen/20261118T1214Z-host-step, job N2.i0158 a1): its end-of-job
//! proof stayed inconclusive for 120 s, "proof pass inconclusive: children [2894482] ended between the listing and the
//! read (hand-off?)". 2894482 was the LEADER of the neighbour attempt N1.i0157 a1 (ledger: started pid 2894482 start
//! 304763968), whose own proof ended (leader reaped and unregistered together) while N2's proof pass was between its
//! listing of the supervisor's children and the reads of their stat: the registry copy taken AFTER the listing no longer
//! named it and the read found it gone (no identity): a hand-off suspect of N2 until the next listings.
//!
//! Here: two overlapping jobs, B launched first, A after (A's start is later than B's launch, so the start-time rule
//! cannot exempt it). Both leaders end; every listing is held 400 ms (test hook `set_listing_hold`); B's proof starts;
//! after a delay A's leader leaves the registry: (i) A's handle is DROPPED (unregistered, its zombie reaped later by the
//! adopted-children reaper), (ii) A's own proof FINISHES (reaped and unregistered together, as on the host), (iii) A
//! leaves a setsid'd daemon that A's own hunt stops and kills (a recorded MEMBER of A, reaped by the reaper). B's proof
//! must never wait for a process of A: no inconclusive round of B names A's leader or A's member.
//!
//! Spawns processes and sends signals: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::launch::{ChildHandle, ChildLauncher};
use rustred_campaign::resources::{read_stat, set_listing_hold};
use std::time::Duration;

/// The whole numbers in `text` (pids are compared as whole tokens, never as substrings).
fn numbers(text: &str) -> Vec<u32> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse().ok())
        .collect()
}

#[test]
fn a_neighbours_process_that_ends_during_this_proof_is_never_its_hand_off_suspect() {
    assert_eq!(std::process::id(), 1, "inside a private pid namespace only");
    let _g = serial();
    let dir = TempDir::new("pf17");
    let mut l = launcher();
    let mut hits = Vec::new();
    let mut slow = Vec::new();
    for k in 0..24u64 {
        let variant = k % 3;
        let delay = Duration::from_millis(150 * (k / 3) + 50);
        let mut b = l
            .launch(&sh(dir.path(), &format!("b{k}"), "exit 0"))
            .unwrap();
        std::thread::sleep(Duration::from_millis(30));
        let dfile = dir.path().join(format!("d{k}.pid"));
        let a_script = if variant == 2 {
            format!(
                "( setsid sh -c 'echo $$ > {f}.tmp && mv {f}.tmp {f}; exec sleep 30' & ); \
                 i=0; while [ ! -s {f} ] && [ $i -lt 200 ]; do sleep 0.05; i=$((i+1)); done; exit 0",
                f = dfile.display()
            )
        } else {
            "exit 0".to_string()
        };
        let mut a = l
            .launch(&sh(dir.path(), &format!("a{k}"), &a_script))
            .unwrap();
        let (bp, ap) = (b.identity().pid, a.identity().pid);
        wait_until("both leaders ended", LONG, || {
            read_stat(bp).is_ok_and(|s| s.is_dead()) && read_stat(ap).is_ok_and(|s| s.is_dead())
        });
        let member = if variant == 2 {
            std::fs::read_to_string(&dfile)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
        } else {
            None
        };
        set_listing_hold(400, 1_000_000);
        if variant == 1 {
            // A's own proof starts FIRST and ends (A reaped and unregistered) while B's passes run, as on the host
            a.poll().unwrap();
            std::thread::sleep(delay * 4);
            b.poll().unwrap();
        } else {
            b.poll().unwrap();
            std::thread::sleep(delay);
        }
        match variant {
            0 => drop(a),
            _ => {
                let ra = drive(&mut a);
                assert!(ra.verification.is_verified(), "A {k}: {ra:?}");
            }
        }
        let r = drive(&mut b);
        set_listing_hold(0, 0);
        assert!(r.verification.is_verified(), "B {k}: {r:?}");
        let of_a: Vec<String> = r
            .proof_waits
            .iter()
            .filter(|w| {
                numbers(&w.example)
                    .iter()
                    .any(|&n| n == ap || Some(n) == member)
            })
            .map(|w| w.example.clone())
            .collect();
        eprintln!(
            "PF17 trial {k} (variant {variant}, delay {delay:?}; A leader {ap}, A member {member:?}): B proof {:.1} s, waits {:?}",
            r.proof_seconds.unwrap_or(0.0),
            r.proof_waits
                .iter()
                .map(|w| (w.kind.as_str(), w.count))
                .collect::<Vec<_>>()
        );
        if !of_a.is_empty() {
            hits.push((k, variant, of_a));
        }
        if r.proof_seconds.unwrap_or(0.0) > 30.0 {
            slow.push((k, r.proof_seconds));
        }
        if let Some(m) = member {
            wait_until("A's daemon gone", Duration::from_secs(30), || {
                read_stat(m).ok().is_none_or(|s| s.is_dead())
            });
        }
    }
    assert!(
        hits.is_empty(),
        "B's proof waited for a process of A: {hits:?}"
    );
    assert!(slow.is_empty(), "{slow:?}");
}
