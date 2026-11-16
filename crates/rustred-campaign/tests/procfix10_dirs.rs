//! Lane procfix7 round 6 (gate10 G10-B1, the listing rule for DIRECTORY listings): every directory listing that decides
//! absence, truncated at entry k for every k (a read that ends early WITHOUT an error: test builds,
//! `fsx::truncate_listings`, this thread only), never decides absence from the shorter listing:
//!
//! - the ledger's create over a directory without MANIFEST.json (`Ledger::create`, site `ledger.listing`): "empty ->
//!   create" is proven by name (MANIFEST.json, COMMIT, segment-000001.jsonl, RECOVERY-PENDING.json): never a new ledger
//!   over a history, never a deletion;
//! - `rebuild_manifest` (site `ledger.listing`): segments 1..N without a gap AND segment N + 1 absent by name: a truncated
//!   listing is refused, never a manifest over a shorter chain.
//!
//! File-level only (no process is started).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
#![cfg(debug_assertions)] // fsx::truncate_listings exists in test builds only
mod ledger_support;

use std::fs;
use std::path::Path;

use ledger_support::*;
use rustred_campaign::fsx;
use rustred_campaign::ledger::{self, Ledger, LedgerConfig, LedgerError};
use rustred_campaign::state::EvidenceLevel;

fn small() -> LedgerConfig {
    LedgerConfig {
        rotate_bytes: 4096,
        ..LedgerConfig::default()
    }
}

fn build(d: &Path, items: usize) -> u64 {
    let clk = clock();
    let mut l = new_ledger(d, as_dyn(&clk), small());
    l.append_all(setup_events("N1")).unwrap();
    for k in 0..items {
        l.append_all(item_life("N1", k, 0, EvidenceLevel::GeometryAccepted, 3))
            .unwrap();
    }
    l.seq()
}

fn names(p: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(p)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn a_truncated_ledger_listing_never_creates_a_ledger_over_a_history() {
    let d = scratch("pf10-dirs-create");
    let _c = Cleanup(d.clone());
    build(&d, 12);
    let ldir = ledger::ledger_dir(&d);
    fs::remove_file(ldir.join(ledger::MANIFEST_FILE)).unwrap(); // a lost manifest
    let before = names(&ldir);
    assert!(before.len() >= 4, "{before:?}");
    for k in 0..=before.len() {
        let r = {
            let _t = fsx::truncate_listings("ledger.listing", k);
            Ledger::create(&d, lock_of(&d), as_dyn(&clock()), small(), genesis())
        };
        match r {
            Err(LedgerError::ManifestMissing { .. }) => {}
            Err(e) => panic!("k {k}: expected ManifestMissing, got {e}"),
            Ok(_) => panic!(
                "k {k}: a NEW ledger was created over a history (listing truncated after {k} entries)"
            ),
        }
        assert_eq!(
            names(&ldir),
            before,
            "k {k}: nothing deleted, nothing added"
        );
    }
}

#[test]
fn a_truncated_ledger_listing_never_rebuilds_a_shorter_chain() {
    let d = scratch("pf10-dirs-rebuild");
    let _c = Cleanup(d.clone());
    let seq = build(&d, 12);
    let ldir = ledger::ledger_dir(&d);
    fs::remove_file(ldir.join(ledger::MANIFEST_FILE)).unwrap();
    let before = names(&ldir);
    let segs = before.iter().filter(|n| n.starts_with("segment-")).count();
    assert!(segs >= 3, "{before:?}");
    // every strictly shorter listing (any order the directory returns) is refused, and nothing is written
    for k in 0..before.len() {
        let r = {
            let _t = fsx::truncate_listings("ledger.listing", k);
            ledger::rebuild_manifest(&d, lock_of(&d))
        };
        match r {
            Err(LedgerError::Corrupt { .. }) => {}
            Err(e) => panic!("k {k}: expected a typed refusal (Corrupt), got {e}"),
            // a truncation that dropped only non-segment entries keeps every segment: the rebuild is then complete
            Ok(rep) => {
                assert_eq!(
                    rep.records, seq,
                    "k {k}: a manifest over a SHORTER chain ({} of {seq} records)",
                    rep.records
                );
                fs::remove_file(ldir.join(ledger::MANIFEST_FILE)).unwrap();
            }
        }
        assert_eq!(names(&ldir), before, "k {k}: nothing left behind");
    }
    // the complete listing rebuilds the whole chain
    let rep = ledger::rebuild_manifest(&d, lock_of(&d)).unwrap();
    assert_eq!(rep.records, seq);
}

/// gate10 m10-6 (mutation M9: a FAILED listing of the ledger directory taken as empty): a ledger directory without a
/// manifest that holds only a file the by-name probes do not know (here `notes.txt`) and whose listing FAILS (EIO, test
/// builds: `fsx::fail_listings`) is never "proven empty": create refuses with the I/O error, nothing is created.
#[test]
fn a_failed_ledger_listing_is_never_empty() {
    let d = scratch("pf10-dirs-failed");
    let _c = Cleanup(d.clone());
    let ldir = ledger::ledger_dir(&d);
    fs::create_dir_all(&ldir).unwrap();
    fs::write(ldir.join("notes.txt"), b"not a ledger file").unwrap();
    let before = names(&ldir);
    let r = {
        let _f = fsx::fail_listings("ledger.listing", libc::EIO);
        Ledger::create(&d, lock_of(&d), as_dyn(&clock()), small(), genesis())
    };
    match r {
        Err(LedgerError::ManifestMissing { .. }) => {
            panic!("a failed listing was read as a listing (ManifestMissing)")
        }
        Err(e) => assert!(
            e.to_string().contains("list") || e.to_string().contains("I/O"),
            "the listing's own error: {e}"
        ),
        Ok(_) => panic!("a NEW ledger was created over a directory whose listing FAILED"),
    }
    assert_eq!(names(&ldir), before, "nothing created");
    // control: the same directory with a working listing is refused as not empty (ManifestMissing)
    assert!(matches!(
        Ledger::create(&d, lock_of(&d), as_dyn(&clock()), small(), genesis()),
        Err(LedgerError::ManifestMissing { .. })
    ));
}
