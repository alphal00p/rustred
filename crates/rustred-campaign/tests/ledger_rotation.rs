//! Segment rotation, manifest, snapshots: state from snapshot + tail == state from full replay; sealed
//! segments immutable and digest-checked; leftovers of a crash during rotation are removed.
mod ledger_support;

use std::fs;
use std::os::unix::fs::PermissionsExt;

use ledger_support::*;
use rustred_campaign::ledger::{self, Event, Ledger, LedgerConfig, LedgerError};
use rustred_campaign::state::{EvidenceLevel, reduce};

fn small() -> LedgerConfig {
    LedgerConfig { rotate_bytes: 8 * 1024, ..LedgerConfig::default() }
}

#[test]
fn snapshot_plus_tail_equals_full_replay() {
    let d = scratch("rot");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), small());
    l.append_all(setup_events("T162")).unwrap();
    for k in 0..60 {
        // one record at a time so rotation happens inside lives
        for e in item_life("T162", k, (k % 3) as u8, EvidenceLevel::IdentityTested, 100 + k as u64) {
            l.append(e).unwrap();
        }
    }
    let m = l.manifest().clone();
    assert!(m.sealed.len() >= 3, "rotated {} times", m.sealed.len());
    let snap = m.snapshot.clone().unwrap();
    assert_eq!(snap.seq, m.sealed.last().unwrap().last_seq);
    let live = l.state().clone();
    let (seq, head) = (l.seq(), l.head().to_string());
    drop(l);
    // open uses the snapshot and replays only the active segment
    let (l2, rep) = Ledger::open(&d, as_dyn(&clk), small()).unwrap();
    assert_eq!(rep.from_snapshot, Some(snap.seq));
    assert_eq!(rep.replayed, seq - snap.seq);
    assert_eq!(l2.state(), &live, "snapshot + tail == live state");
    assert_eq!((l2.seq(), l2.head()), (seq, head.as_str()));
    // full replay from record 1 (no snapshot) gives the same state
    let chain = ledger::verify_chain(&d).unwrap();
    assert_eq!(chain.state, live);
    assert_eq!(chain.snapshot_matches, Some(true));
    assert_eq!(chain.records, seq);
    let recs = ledger::read_records(&d).unwrap();
    assert_eq!(reduce(&recs).unwrap(), live);
    // sealed segments are read-only and SegmentOpened records name the previous digest
    for s in &m.sealed {
        let mode = fs::metadata(ledger::ledger_dir(&d).join(&s.file)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o444, "{}", s.file);
    }
    let opened: Vec<_> = recs.iter().filter_map(|r| if let Event::SegmentOpened { previous_blake3, .. } = &r.event { Some(previous_blake3.clone()) } else { None }).collect();
    let sealed_digests: Vec<_> = m.sealed.iter().map(|s| s.blake3.clone()).collect();
    assert_eq!(opened, sealed_digests);
    // at most keep_old_snapshots + 1 snapshot files
    let snaps = fs::read_dir(ledger::ledger_dir(&d)).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("snapshot-")).count();
    assert!(snaps <= 2, "{snaps} snapshots kept");
}

#[test]
fn edited_sealed_segment_is_detected() {
    let d = scratch("rotedit");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), small());
    l.append_all(setup_events("T162")).unwrap();
    for k in 0..30 {
        l.append_all(item_life("T162", k, 0, EvidenceLevel::IdentityTested, 10)).unwrap();
    }
    let first = l.manifest().sealed[0].file.clone();
    drop(l);
    let p = ledger::ledger_dir(&d).join(&first);
    fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
    let mut b = fs::read(&p).unwrap();
    let n = b.len();
    b.truncate(n - 10); // truncation of a sealed segment
    fs::write(&p, &b).unwrap();
    match ledger::verify_chain(&d) {
        Err(LedgerError::Corrupt { file, why, .. }) => {
            assert_eq!(file, first);
            assert!(why.contains("digest"), "{why}");
        }
        other => panic!("expected Corrupt, got {other:?}"),
    }
    // open with a snapshot does not read sealed segments (fast path); verify does
    assert!(Ledger::open(&d, as_dyn(&clk), small()).is_ok());
}

#[test]
fn crash_during_rotation_leaves_removable_leftovers() {
    let d = scratch("rotcrash");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(scenario()).unwrap();
    let digest = l.state().digest();
    let m = l.manifest().clone();
    drop(l);
    let ld = ledger::ledger_dir(&d);
    // a rotation that died after writing the snapshot and creating the new segment, before the manifest
    fs::write(ld.join(format!("snapshot-{:012}.json", 999_999)), b"{}").unwrap();
    fs::write(ld.join(format!("segment-{:06}.jsonl", m.active.index + 1)), b"").unwrap();
    fs::write(ld.join(".MANIFEST.json.rr-tmp-1-0"), b"partial").unwrap();
    let (l2, rep) = Ledger::open(&d, as_dyn(&clk), LedgerConfig::default()).unwrap();
    assert_eq!(rep.removed_unreferenced.len(), 2, "{:?}", rep.removed_unreferenced);
    assert_eq!(l2.state().digest(), digest);
    assert!(!ld.join(".MANIFEST.json.rr-tmp-1-0").exists());
    drop(l2);
    // a NON-empty unreferenced segment is corruption
    fs::write(ld.join(format!("segment-{:06}.jsonl", m.active.index + 1)), b"x\n").unwrap();
    assert!(matches!(Ledger::open(&d, as_dyn(&clk), LedgerConfig::default()), Err(LedgerError::Corrupt { .. })));
}
