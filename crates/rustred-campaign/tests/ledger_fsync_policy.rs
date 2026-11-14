//! The FSYNC POLICY itself, observed through the `FileOps` seam (audit A n1: the SIGKILL crash test would pass
//! with fsync removed). A recording fake logs every write / fsync / rename / directory fsync; for each durability
//! class the exact order of operations before `append` returns is asserted. A fault-injecting fake fails the Nth
//! operation (EIO / ENOSPC) and drives the M1 / M2 / disk-full paths deterministically.
mod ledger_support;

use std::sync::Arc;

use ledger_support::*;
use rustred_campaign::ledger::{Event, LedgerConfig, LedgerError, OpenOptions};
use rustred_campaign::state::{CampaignEvent, EvidenceLevel};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn drain() -> Event {
    Event::Campaign {
        ev: CampaignEvent::Drain,
        reason: "x".into(),
    }
}

#[test]
fn order_of_operations_per_durability_class() {
    let d = scratch("fsync-order");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let ops = Ops::new();
    let cfg = LedgerConfig {
        lazy_max_records: 3,
        lazy_max_ms: u64::MAX,
        ..LedgerConfig::default()
    };
    let mut l =
        new_ledger_with(&d, as_dyn(&clk), cfg, Arc::new(RecordingOps(ops.clone()))).unwrap();
    // create: segment + record 1 fsynced + mark, directory fsync, THEN the manifest (the commit point)
    // (the fsync of the campaign directory after creating `ledger/` is filtered: its name is the scratch dir)
    let log: Vec<String> = ops
        .take_log()
        .into_iter()
        .filter(|x| !x.contains("rr-ledger-"))
        .collect();
    assert_eq!(
        log,
        s(&[
            "write_at COMMIT",
            "open segment-000001.jsonl",
            "append segment-000001.jsonl",
            "fsync segment-000001.jsonl",
            "write_at COMMIT",
            "fsync COMMIT",
            "fsync-dir ledger",
            "write+fsync tmp(MANIFEST.json)",
            "rename tmp(MANIFEST.json) -> MANIFEST.json",
            "fsync-dir ledger",
        ])
    );
    l.append_all(setup_events("T162")).unwrap();
    ops.take_log();
    // SYNC record: one write, fsync of the segment, then the commit mark written and fsynced, then return
    l.append(drain()).unwrap();
    let sync_order = s(&[
        "append segment-000001.jsonl",
        "fsync segment-000001.jsonl",
        "write_at COMMIT",
        "fsync COMMIT",
    ]);
    assert_eq!(ops.take_log(), sync_order);
    // LAZY records: one write each, no fsync ...
    l.append(Event::Note { text: "1".into() }).unwrap();
    l.append(Event::Note { text: "2".into() }).unwrap();
    assert_eq!(
        ops.take_log(),
        s(&["append segment-000001.jsonl", "append segment-000001.jsonl"])
    );
    // ... until lazy_max_records: then the same order as a sync record
    l.append(Event::Note { text: "3".into() }).unwrap();
    assert_eq!(ops.take_log(), sync_order);
    // group commit: a batch is ONE write and ONE fsync if any record is sync
    l.append_all(item_life("T162", 0, 0, EvidenceLevel::IdentityTested, 5))
        .unwrap();
    assert_eq!(ops.take_log(), sync_order);
    // explicit sync of lazy records
    l.append(Event::Note { text: "4".into() }).unwrap();
    l.sync().unwrap();
    assert_eq!(
        ops.take_log(),
        s(&[
            "append segment-000001.jsonl",
            "fsync segment-000001.jsonl",
            "write_at COMMIT",
            "fsync COMMIT"
        ])
    );
    // rotation: fsync active + mark; snapshot (temp, fsync, rename, dir fsync); new segment + dir fsync;
    // manifest (temp, fsync, rename, dir fsync) = commit point; segment_opened in the new segment (sync)
    l.rotate().unwrap();
    assert_eq!(
        ops.take_log(),
        s(&[
            "fsync segment-000001.jsonl",
            "write_at COMMIT",
            "fsync COMMIT",
            "write+fsync tmp(snapshot)",
            "rename tmp(snapshot) -> snapshot",
            "fsync-dir ledger",
            "open segment-000002.jsonl",
            "fsync-dir ledger",
            "write+fsync tmp(MANIFEST.json)",
            "rename tmp(MANIFEST.json) -> MANIFEST.json",
            "fsync-dir ledger",
            "append segment-000002.jsonl",
            "fsync segment-000002.jsonl",
            "write_at COMMIT",
            "fsync COMMIT",
        ])
    );
}

/// Fail each operation of a SYNC append in turn: segment write / fsync -> typed error, poisoned, nothing
/// acknowledged, the reopened ledger does not hold the record; commit-mark write / fsync (audit-ledger2 m7) ->
/// typed error, poisoned, NOT acknowledged although the record is durable: the reopened ledger holds it (the
/// ledger is the truth), and no acknowledged record ever lies beyond the on-disk mark.
#[test]
fn faults_at_every_operation_of_a_sync_append() {
    for (n, errno, durable) in [
        (1, libc::EIO, false),    // append segment
        (1, libc::ENOSPC, false), // append segment, disk full
        (2, libc::EIO, false),    // fsync segment (M1)
        (3, libc::EIO, true),     // write_at COMMIT
        (4, libc::EIO, true),     // fsync COMMIT
        (4, libc::ENOSPC, true),  // fsync COMMIT, disk full: typed NoSpace (audit-ledger2 m1)
    ] {
        let d = scratch("fsync-fault");
        let _c = Cleanup(d.clone());
        let clk = clock();
        let ops = Ops::new();
        let mut l = new_ledger_with(
            &d,
            as_dyn(&clk),
            LedgerConfig::default(),
            Arc::new(RecordingOps(ops.clone())),
        )
        .unwrap();
        l.append_all(setup_events("T162")).unwrap();
        let seq = l.seq();
        ops.take_log();
        ops.arm(n, errno, "");
        let r = l.append(drain());
        ops.disarm();
        match (&r, errno) {
            (Err(LedgerError::NoSpace { .. }), libc::ENOSPC) => {}
            (Err(LedgerError::Io { .. }), libc::EIO) => {}
            other => panic!("op {n}: {other:?}"),
        }
        assert!(l.is_poisoned(), "op {n}");
        assert_eq!(l.seq(), if durable { seq + 1 } else { seq }, "op {n}");
        assert!(matches!(l.sync(), Err(LedgerError::Poisoned { .. })));
        drop(l);
        let (l2, rep) = open_ledger_with(
            &d,
            as_dyn(&clk),
            LedgerConfig::default(),
            Arc::new(RecordingOps(ops.clone())),
            OpenOptions::default(),
        )
        .unwrap_or_else(|e| panic!("op {n}: reopen: {e}"));
        assert_eq!(
            l2.seq(),
            if durable { seq + 1 } else { seq },
            "op {n}: {rep:?}"
        );
        if n == 3 {
            // the record was beyond the (old) mark: re-written and fsynced by the fsync gate on open
            assert!(rep.regated_bytes > 0, "op {n}");
        }
    }
}

/// Fail each operation of a rotation in turn: before the manifest rename the append is acknowledged, a warning
/// is kept and the ledger is unchanged; after it (directory fsync of the manifest, the new segment's first record)
/// the handle is poisoned. Every case reopens to a valid ledger holding every acknowledged record.
#[test]
fn faults_at_every_operation_of_a_rotation() {
    let cfg = LedgerConfig {
        rotate_bytes: 6 * 1024,
        rotation_retry_ms: u64::MAX,
        ..LedgerConfig::default()
    };
    // operations of a rotation triggered by an append (after its own 4: append, fsync, mark, mark fsync)
    let names = [
        "fsync segment",
        "write_at COMMIT",
        "fsync COMMIT",
        "write+fsync tmp(snapshot)",
        "rename tmp(snapshot)",
        "fsync-dir ledger",
        "open segment-000002",
        "fsync-dir ledger",
        "write+fsync tmp(MANIFEST",
        "rename tmp(MANIFEST",
        "fsync-dir ledger",
        "append segment-000002",
    ];
    for k in 0..names.len() {
        let d = scratch("rot-fault");
        let _c = Cleanup(d.clone());
        let clk = clock();
        let ops = Ops::new();
        let mut l = new_ledger_with(
            &d,
            as_dyn(&clk),
            cfg.clone(),
            Arc::new(RecordingOps(ops.clone())),
        )
        .unwrap();
        l.append_all(setup_events("T162")).unwrap();
        // a sync record whose length crosses the rotation size exactly
        let seg = rustred_campaign::ledger::ledger_dir(&d).join(&l.manifest().active.file);
        let len = std::fs::metadata(&seg).unwrap().len() as usize;
        let reason = "x".repeat(6 * 1024 - len);
        assert!(l.manifest().sealed.is_empty());
        ops.take_log();
        ops.arm(4 + k as i64 + 1, libc::EIO, "");
        let r = l.append(Event::Campaign {
            ev: CampaignEvent::Drain,
            reason: reason.clone(),
        });
        ops.disarm();
        let log = ops.take_log();
        assert!(
            log[4 + k].starts_with(names[k]),
            "op {k}: expected {} got {:?}",
            names[k],
            log.get(4 + k)
        );
        assert!(
            r.is_ok(),
            "op {k} ({}): the record was durable: {r:?}",
            names[k]
        );
        let after_rename = k >= 10;
        let mark_only = k == 1 || k == 2;
        if after_rename {
            assert!(l.is_poisoned(), "op {k}: after the manifest rename");
        } else if !mark_only && k != 0 {
            assert!(!l.is_poisoned(), "op {k}");
            assert!(
                l.manifest().sealed.is_empty(),
                "op {k}: no rotation happened"
            );
        }
        assert!(!l.warnings().is_empty(), "op {k}");
        let seq = l.seq();
        drop(l);
        let (l2, _) = open_ledger(&d, as_dyn(&clk), cfg.clone())
            .unwrap_or_else(|e| panic!("op {k} ({}): reopen: {e}", names[k]));
        assert!(l2.seq() >= seq, "op {k}: no acknowledged record lost");
        assert!(rustred_campaign::ledger::verify_chain(&d).is_ok(), "op {k}");
    }
}
