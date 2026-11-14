//! Lane int5, item 3: `rr_campaign rebuild-manifest CDIR` (ledgerfix2's recovery verb `ledger::rebuild_manifest` on the
//! command line). Refused while the campaign lock is held (a supervisor) or when a manifest exists; otherwise the
//! manifest is rebuilt from the verified chain and the ledger reads back unchanged. The CLI spawns nothing and signals
//! nothing (no namespace needed; the suite runs it under LIGHT_PIDNS=1 anyway).

mod ledger_support;

use ledger_support::*;
use rustred_campaign::clock::SystemClock;
use rustred_campaign::ledger::{self, Ledger, LedgerConfig};
use rustred_campaign::lock;
use rustred_campaign::state::EvidenceLevel;
use std::path::Path;
use std::process::Command;

const SUP: &str = env!("CARGO_BIN_EXE_rr_campaign");

fn cli(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(SUP).args(args).output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// A ledger of several segments written under a lock of our own, released afterwards (as a stopped supervisor).
fn build(d: &Path) -> u64 {
    let (lk, _) = lock::acquire(d, &SystemClock::new(), &["test".into()]).unwrap();
    let cfg = LedgerConfig {
        rotate_bytes: 4096,
        ..LedgerConfig::default()
    };
    let mut l = Ledger::create(d, &lk, as_dyn(&clock()), cfg, genesis()).unwrap();
    l.append_all(setup_events("N1")).unwrap();
    for k in 0..10 {
        l.append_all(item_life("N1", k, 0, EvidenceLevel::GeometryAccepted, 3))
            .unwrap();
    }
    let seq = l.seq();
    drop(l);
    lk.release().unwrap();
    seq
}

#[test]
fn rebuild_manifest_verb_refuses_under_a_held_lock_and_rebuilds_the_verified_chain() {
    let d = scratch("int5-rebuild");
    let _c = Cleanup(d.clone());
    let seq = build(&d);
    let before = ledger::read_records(&d).unwrap();
    let ds = d.to_str().unwrap();
    let manifest = ledger::ledger_dir(&d).join(ledger::MANIFEST_FILE);
    // a manifest present: nothing to rebuild
    let (code, _, err) = cli(&["rebuild-manifest", ds]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("nothing to rebuild"), "{err}");
    std::fs::remove_file(&manifest).unwrap();
    // a live holder of the campaign lock (here: this process, as a supervisor would be): refused, nothing written
    {
        let (held, _) = lock::acquire(&d, &SystemClock::new(), &["holder".into()]).unwrap();
        let (code, _, err) = cli(&["rebuild-manifest", ds]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("lock"), "{err}");
        assert!(!manifest.exists());
        held.release().unwrap();
    }
    // free: rebuilt from the verified chain; the ledger reads back unchanged
    let (code, out, err) = cli(&["rebuild-manifest", ds]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("rebuilt") && out.contains(&format!("{seq} record(s)")), "{out}");
    assert!(manifest.exists());
    assert_eq!(ledger::read_records(&d).unwrap(), before);
    let v = ledger::open_read_only(&d).unwrap();
    assert_eq!(v.seq, seq);
    // the lock was released (a supervisor can start): no owner record left
    assert!(!d.join("LOCK.json").exists());
    // usage / not found
    assert_eq!(cli(&["rebuild-manifest"]).0, 2);
    let missing = d.join("no-such-campaign");
    assert_eq!(cli(&["rebuild-manifest", missing.to_str().unwrap()]).0, 3);
}
