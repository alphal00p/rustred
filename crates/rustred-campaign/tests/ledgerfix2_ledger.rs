//! Lane ledgerfix2: file-level tests of the re-audit fixes: the M1 recovery verb, deleted segments (m8), the
//! fsync gate before the recovery record (m11), background snapshots (m10), an older snapshot version, and a
//! crash-point enumeration of the SUPERVISOR'S START DECISION ("MANIFEST.json present -> open, else create") over
//! create / appends / rotations: no crash point may make a directory with an acknowledged history look new.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
mod ledger_support;

use std::fs;
use std::io;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use ledger_support::*;
use rustred_campaign::clock::{Clock, ManualClock};
use rustred_campaign::ledger::{
    self, Event, FileOps, Genesis, Ledger, LedgerConfig, LedgerError, OpenOptions, SegmentFile,
};
use rustred_campaign::lock;
use rustred_campaign::state::EvidenceLevel;

fn small() -> LedgerConfig {
    LedgerConfig {
        rotate_bytes: 4096,
        ..LedgerConfig::default()
    }
}

fn build(d: &Path, cfg: LedgerConfig, items: usize) -> Ledger {
    let clk = clock();
    let mut l = new_ledger(d, as_dyn(&clk), cfg);
    l.append_all(setup_events("N1")).unwrap();
    for k in 0..items {
        l.append_all(item_life("N1", k, 0, EvidenceLevel::GeometryAccepted, 3))
            .unwrap();
    }
    l
}

fn segments(d: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(ledger::ledger_dir(d))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("segment-"))
        .collect();
    v.sort();
    v
}

#[test]
fn m1_manifest_missing_is_refused_and_rebuilt_from_the_verified_chain() {
    let d = scratch("lf2-rebuild");
    let _c = Cleanup(d.clone());
    let l = build(&d, small(), 12);
    let seq = l.seq();
    drop(l);
    let before = ledger::read_records(&d).unwrap();
    let segs = segments(&d);
    assert!(segs.len() >= 3);
    let ldir = ledger::ledger_dir(&d);
    fs::remove_file(ldir.join(ledger::MANIFEST_FILE)).unwrap();
    // create refuses (typed) and deletes nothing
    let e = Ledger::create(&d, lock_of(&d), as_dyn(&clock()), small(), genesis()).unwrap_err();
    assert!(matches!(e, LedgerError::ManifestMissing { .. }), "{e}");
    assert!(e.to_string().contains("damaged ledger"), "{e}");
    assert_eq!(segments(&d), segs, "nothing deleted");
    // the recovery verb: verified chain -> manifest (no snapshot)
    let r = ledger::rebuild_manifest(&d, lock_of(&d)).unwrap();
    assert_eq!(r.records, seq);
    assert_eq!(r.sealed as usize, segs.len() - 1);
    assert!(matches!(
        ledger::rebuild_manifest(&d, lock_of(&d)),
        Err(LedgerError::AlreadyExists { .. })
    ));
    assert_eq!(ledger::read_records(&d).unwrap(), before);
    let (l, rep) = open_ledger(&d, as_dyn(&clock()), small()).unwrap();
    assert_eq!(l.seq(), seq);
    assert_eq!(
        rep.from_snapshot, None,
        "the rebuilt manifest names no snapshot"
    );
    assert_eq!(ledger::verify_chain(&d).unwrap().state, *l.state());
}

#[test]
fn m1_rebuild_refuses_a_gap_or_a_damaged_chain() {
    let d = scratch("lf2-rebuild-bad");
    let _c = Cleanup(d.clone());
    drop(build(&d, small(), 12));
    let ldir = ledger::ledger_dir(&d);
    fs::remove_file(ldir.join(ledger::MANIFEST_FILE)).unwrap();
    let segs = segments(&d);
    // a damaged byte inside a sealed segment
    let p = ldir.join(&segs[1]);
    let mut perm = fs::metadata(&p).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    fs::set_permissions(&p, perm).unwrap();
    let mut b = fs::read(&p).unwrap();
    b[40] ^= 0x02;
    fs::write(&p, &b).unwrap();
    assert!(matches!(
        ledger::rebuild_manifest(&d, lock_of(&d)),
        Err(LedgerError::Corrupt { .. })
    ));
    b[40] ^= 0x02;
    fs::write(&p, &b).unwrap();
    // a missing middle segment
    fs::rename(&p, ldir.join("moved-away")).unwrap();
    assert!(matches!(
        ledger::rebuild_manifest(&d, lock_of(&d)),
        Err(LedgerError::Corrupt { .. })
    ));
    assert!(!ldir.join(ledger::MANIFEST_FILE).exists());
}

#[test]
fn m1_crashed_create_residue_is_still_recreated() {
    // procfix7 round 5 (gate9 G9-B1): the residue of a create that died before its manifest is NO LONGER deleted and
    // recreated: a ledger directory with files and no MANIFEST is refused (ManifestMissing, nothing deleted), and the
    // documented recovery (rebuild-manifest from the verified chain) restores it. The name is kept for the history of
    // ledgerfix2 m1; the behaviour is the refuse-not-repair rule.
    let d = scratch("lf2-residue");
    let _c = Cleanup(d.clone());
    drop(new_ledger(&d, as_dyn(&clock()), small())); // record 1 + mark + manifest
    let ldir = ledger::ledger_dir(&d);
    fs::remove_file(ldir.join(ledger::MANIFEST_FILE)).unwrap(); // = a create that died before its manifest
    let names = |p: &Path| -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(p)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    };
    let before = names(&ldir);
    assert!(!before.is_empty());
    match Ledger::create(&d, lock_of(&d), as_dyn(&clock()), small(), genesis()) {
        Err(LedgerError::ManifestMissing { .. }) => {}
        Err(e) => panic!("create over a residue: expected ManifestMissing, got {e}"),
        Ok(_) => panic!("create over a residue must be refused (never a repair by deletion)"),
    }
    assert_eq!(names(&ldir), before, "nothing deleted, nothing added");
    ledger::rebuild_manifest(&d, lock_of(&d)).unwrap();
    let (l, _) = open_ledger(&d, as_dyn(&clock()), small()).unwrap();
    assert_eq!(l.seq(), 1);
}

#[test]
fn m8_a_deleted_segment_is_corrupt_for_every_reader() {
    for which in ["sealed", "active"] {
        let d = scratch("lf2-deleted");
        let _c = Cleanup(d.clone());
        drop(build(&d, small(), 12));
        let ldir = ledger::ledger_dir(&d);
        let m = ledger::read_manifest(&ldir).unwrap();
        let f = if which == "sealed" {
            m.sealed[m.sealed.len() - 1].file.clone()
        } else {
            m.active.file.clone()
        };
        fs::remove_file(ldir.join(&f)).unwrap();
        let corrupt = |r: Result<(), LedgerError>, what: &str| match r {
            Err(LedgerError::Corrupt { .. }) => {}
            other => panic!("{which} {what}: {other:?}"),
        };
        corrupt(
            open_ledger(&d, as_dyn(&clock()), small()).map(|_| ()),
            "open",
        );
        corrupt(ledger::open_read_only(&d).map(|_| ()), "open_read_only");
        corrupt(ledger::verify_chain(&d).map(|_| ()), "verify_chain");
        corrupt(ledger::read_records(&d).map(|_| ()), "read_records");
    }
}

#[test]
fn m11_fsync_gate_precedes_the_recovery_record() {
    let d = scratch("lf2-gate");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let ops = Ops::new();
    let cfg = LedgerConfig {
        lazy_max_records: 10_000,
        lazy_max_ms: 1 << 40,
        ..LedgerConfig::default()
    };
    let mut l = new_ledger_with(
        &d,
        as_dyn(&clk),
        cfg.clone(),
        Arc::new(RecordingOps(ops.clone())),
    )
    .unwrap();
    l.append_all(setup_events("N1")).unwrap();
    for k in 0..3 {
        l.append(Event::Note {
            text: format!("lazy {k}"),
        })
        .unwrap();
    }
    let seg = ledger::ledger_dir(&d).join(&l.manifest().active.file);
    drop(l);
    let f = fs::OpenOptions::new().append(true).open(&seg).unwrap();
    f.write_all_at(b"{\"schema\":\"torn", f.metadata().unwrap().len())
        .unwrap();
    ops.take_log();
    let (_l, rep) = open_ledger_with(
        &d,
        as_dyn(&clk),
        cfg,
        Arc::new(RecordingOps(ops.clone())),
        OpenOptions::default(),
    )
    .unwrap();
    assert!(rep.recovery.is_some() && rep.regated_bytes > 0, "{rep:?}");
    let log = ops.take_log();
    let pos = |what: &str| {
        log.iter()
            .position(|o| o == what)
            .unwrap_or_else(|| panic!("{what} not in {log:?}"))
    };
    let gate = pos("write_at segment-000001.jsonl");
    let recovery = pos("append segment-000001.jsonl");
    assert!(
        gate < recovery && log[gate + 1] == "fsync segment-000001.jsonl",
        "the gate (rewrite + fsync of the bytes beyond the mark) must precede the recovery record: {log:?}"
    );
}

#[test]
fn m10_background_snapshots_never_block_and_are_used_one_rotation_later() {
    let d = scratch("lf2-bg");
    let _c = Cleanup(d.clone());
    let cfg = LedgerConfig {
        rotate_bytes: 4096,
        background_snapshot_min_entries: 0,
        ..LedgerConfig::default()
    };
    let mut l = build(&d, cfg.clone(), 40);
    assert!(l.manifest().sealed.len() >= 5);
    // the manifest names the snapshot of an EARLIER seal point (written in the background)
    let named = l.wait_snapshot();
    l.rotate().unwrap();
    let m = l.manifest().clone();
    let snap = m.snapshot.clone().expect("a background snapshot is named");
    if let Some(s) = named {
        assert_eq!(
            snap.seq, s,
            "the finished snapshot is named by the next rotation"
        );
    }
    assert!(snap.seq < m.sealed.last().unwrap().last_seq);
    let seq = l.seq();
    let st = l.state().clone();
    drop(l); // orderly close: the snapshot started by this rotation is finished and named
    let closed = ledger::read_manifest(&ledger::ledger_dir(&d))
        .unwrap()
        .snapshot
        .unwrap();
    assert!(closed.seq > snap.seq, "the close names the newest snapshot");
    assert_eq!(closed.seq, m.sealed.last().unwrap().last_seq);
    let (l, rep) = open_ledger(&d, as_dyn(&clock()), cfg.clone()).unwrap();
    assert_eq!(rep.from_snapshot, Some(closed.seq));
    assert!(rep.replayed > 0);
    assert_eq!((l.seq(), l.state()), (seq, &st));
    let chain = ledger::verify_chain(&d).unwrap();
    assert_eq!(chain.snapshot_matches, Some(true));
    assert_eq!(chain.state, st);
    assert_eq!(ledger::open_read_only(&d).unwrap().state, st);
    // a snapshot named one rotation late: the replay covers the sealed segments after it (checked above through
    // `snap`); dropping a handle while a snapshot is written leaves a directory that opens cleanly
    let mut l = l;
    for k in 40..60 {
        l.append_all(item_life("N1", k, 0, EvidenceLevel::GeometryAccepted, 3))
            .unwrap();
    }
    let st = l.state().clone();
    drop(l);
    let (l, _) = open_ledger(&d, as_dyn(&clock()), cfg).unwrap();
    assert_eq!(*l.state(), st);
    assert_eq!(ledger::verify_chain(&d).unwrap().state, st);
}

#[test]
fn m10_an_older_snapshot_version_is_ignored_full_replay() {
    let d = scratch("lf2-v1");
    let _c = Cleanup(d.clone());
    let l = build(&d, small(), 12);
    let st = l.state().clone();
    drop(l);
    let ldir = ledger::ledger_dir(&d);
    let m = ledger::read_manifest(&ldir).unwrap();
    let p = ldir.join(&m.snapshot.as_ref().unwrap().file);
    let b = fs::read(&p).unwrap();
    let old = String::from_utf8(b).unwrap().replacen(
        "rustred.campaign.snapshot.v2",
        "rustred.campaign.snapshot.v1",
        1,
    );
    fs::write(&p, old).unwrap();
    let (l, rep) = open_ledger(&d, as_dyn(&clock()), small()).unwrap();
    assert_eq!(rep.from_snapshot, None);
    assert!(
        rep.snapshot_note
            .as_deref()
            .is_some_and(|n| n.contains("snapshot.v1")),
        "{rep:?}"
    );
    assert_eq!(*l.state(), st);
    assert_eq!(ledger::verify_chain(&d).unwrap().snapshot_matches, None);
    assert_eq!(ledger::open_read_only(&d).unwrap().state, st);
}

// ------------------------------------------------------------------------------------------------------------
// crash-point enumeration of the supervisor's start decision
// ------------------------------------------------------------------------------------------------------------

/// Every operation counts one event; the n-th kills the "process": a write lands half, later operations fail.
struct Kill {
    left: AtomicI64,
    dead: AtomicBool,
}

impl Kill {
    fn step(&self) -> bool {
        if self.dead.load(Ordering::SeqCst) {
            return false;
        }
        if self.left.fetch_sub(1, Ordering::SeqCst) <= 1 {
            self.dead.store(true, Ordering::SeqCst);
            return false;
        }
        true
    }
}

fn killed() -> io::Error {
    io::Error::other("killed")
}

struct KFile {
    f: fs::File,
    len: u64,
    k: Arc<Kill>,
}

impl SegmentFile for KFile {
    fn append(&mut self, b: &[u8]) -> io::Result<()> {
        if !self.k.step() {
            let _ = self.f.write_all_at(&b[..b.len() / 2], self.len);
            return Err(killed());
        }
        self.f.write_all_at(b, self.len)?;
        self.len += b.len() as u64;
        Ok(())
    }
    fn sync(&mut self) -> io::Result<()> {
        if !self.k.step() {
            return Err(killed());
        }
        Ok(())
    }
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        if !self.k.step() {
            return Err(killed());
        }
        self.f.set_len(len)?;
        self.len = len;
        Ok(())
    }
    fn write_at(&mut self, off: u64, b: &[u8]) -> io::Result<()> {
        if !self.k.step() {
            let _ = self.f.write_all_at(&b[..b.len() / 2], off);
            return Err(killed());
        }
        self.f.write_all_at(b, off)?;
        self.len = self.len.max(off + b.len() as u64);
        Ok(())
    }
}

struct KOps(Arc<Kill>);

impl KOps {
    fn open(&self, p: &Path) -> io::Result<Box<dyn SegmentFile>> {
        if !self.0.step() {
            return Err(killed());
        }
        let f = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(p)?;
        let len = f.metadata()?.len();
        Ok(Box::new(KFile {
            f,
            len,
            k: self.0.clone(),
        }))
    }
}

impl FileOps for KOps {
    fn open_segment(&self, p: &Path) -> io::Result<Box<dyn SegmentFile>> {
        self.open(p)
    }
    fn open_mark(&self, p: &Path) -> io::Result<Box<dyn SegmentFile>> {
        self.open(p)
    }
    fn write_new_synced(&self, p: &Path, b: &[u8]) -> io::Result<()> {
        if !self.0.step() {
            let _ = fs::write(p, &b[..b.len() / 2]);
            return Err(killed());
        }
        ledger::RealOps.write_new_synced(p, b)
    }
    fn rename(&self, a: &Path, b: &Path) -> io::Result<()> {
        if !self.0.step() {
            return Err(killed());
        }
        fs::rename(a, b)
    }
    fn sync_dir(&self, _d: &Path) -> io::Result<()> {
        if !self.0.step() {
            return Err(killed());
        }
        Ok(())
    }
    fn remove(&self, p: &Path) -> io::Result<()> {
        if !self.0.step() {
            return Err(killed());
        }
        fs::remove_file(p)
    }
}

/// One supervisor session up to `limit` file operations: lock, start decision, its first records, a workload
/// with rotations. Returns the highest acknowledged seq.
fn session(d: &Path, limit: i64, items: usize) -> (u64, bool) {
    let k = Arc::new(Kill {
        left: AtomicI64::new(limit),
        dead: AtomicBool::new(false),
    });
    let ops: Arc<dyn FileOps> = Arc::new(KOps(k.clone()));
    let clk: Arc<dyn Clock> = Arc::new(ManualClock::at(T0).with_tick(100));
    let (lk, _) = lock::acquire(d, clk.as_ref(), &["sup".into()]).unwrap();
    let manifest = ledger::ledger_dir(d).join(ledger::MANIFEST_FILE);
    // THE SUPERVISOR'S DECISION (supervisor/mod.rs start): manifest present -> open, else create
    let r = if manifest.exists() {
        Ledger::open_with(d, &lk, clk, small(), ops, OpenOptions::default()).map(|(l, _)| l)
    } else {
        Ledger::create_with(
            d,
            &lk,
            clk,
            small(),
            ops,
            Genesis::new("c", &digest("s"), 9),
        )
    };
    let mut l = match r {
        Ok(l) => l,
        Err(_) => return (0, k.dead.load(Ordering::SeqCst)),
    };
    let mut acked = l.seq();
    let mut batches = vec![setup_events("N1")];
    for i in 0..items {
        batches.push(item_life("N1", i, 0, EvidenceLevel::GeometryAccepted, 1));
    }
    for b in batches {
        match l.append_all(b) {
            Ok(q) => acked = *q.last().unwrap(),
            Err(_) => break,
        }
    }
    (acked, k.dead.load(Ordering::SeqCst))
}

#[test]
fn start_path_crash_points_never_make_a_history_look_new() {
    let base = scratch("lf2-start");
    let _c = Cleanup(base.clone());
    let full = base.join("full");
    fs::create_dir_all(&full).unwrap();
    let (acked_full, dead) = session(&full, i64::MAX, 10);
    assert!(!dead && acked_full > 20);
    assert!(
        ledger::read_manifest(&ledger::ledger_dir(&full))
            .unwrap()
            .sealed
            .len()
            >= 3
    );
    let mut fails = Vec::new();
    let (mut new_after, mut opened_after) = (0, 0);
    let mut n = 1;
    loop {
        let d = base.join(format!("k{n}"));
        fs::create_dir_all(&d).unwrap();
        let (acked, dead) = session(&d, n, 10);
        if !dead {
            break; // beyond the last operation
        }
        let manifest = ledger::ledger_dir(&d).join(ledger::MANIFEST_FILE).exists();
        // the next start, with the real operations
        let (lk, _) = lock::acquire(
            &d,
            &rustred_campaign::clock::SystemClock::new(),
            &["x".into()],
        )
        .unwrap();
        if manifest {
            opened_after += 1;
            match Ledger::open(&d, &lk, as_dyn(&clock()), small()) {
                Ok((l, _)) if l.seq() >= acked => {}
                Ok((l, _)) => fails.push(format!(
                    "op {n}: reopened at {} < acknowledged {acked}",
                    l.seq()
                )),
                Err(e) => fails.push(format!("op {n}: reopen refused: {e}")),
            }
        } else {
            new_after += 1;
            if acked > 1 {
                fails.push(format!(
                    "op {n}: NO MANIFEST although seq {acked} was acknowledged"
                ));
            }
            // procfix7 round 5 (gate9 G9-B1): create works into an absent or EMPTY ledger directory only; a residue
            // is refused with nothing deleted and recovered by rebuild-manifest, or, without a complete record 1
            // (rebuild refuses; nothing was acknowledged), removed by hand before the new create
            let ldir = ledger::ledger_dir(&d);
            let names = || -> Vec<String> {
                let mut v: Vec<String> = fs::read_dir(&ldir)
                    .map(|r| {
                        r.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                v.sort();
                v
            };
            let before = names();
            match Ledger::create(&d, &lk, as_dyn(&clock()), small(), genesis()) {
                Ok(_) if before.is_empty() => {}
                Ok(_) => fails.push(format!("op {n}: create over the residue {before:?}")),
                Err(LedgerError::ManifestMissing { .. }) => {
                    if names() != before {
                        fails.push(format!("op {n}: a refused create changed the residue"));
                    }
                    match ledger::rebuild_manifest(&d, &lk) {
                        Ok(_) => match Ledger::open(&d, &lk, as_dyn(&clock()), small()) {
                            Ok((l, _)) if l.seq() >= acked => {}
                            Ok((l, _)) => fails.push(format!(
                                "op {n}: rebuilt at {} < acknowledged {acked}",
                                l.seq()
                            )),
                            Err(e) => fails.push(format!("op {n}: open after rebuild: {e}")),
                        },
                        Err(_) if acked == 0 => {
                            let _ = fs::remove_dir_all(&ldir); // the operator's removal by hand
                            if let Err(e) =
                                Ledger::create(&d, &lk, as_dyn(&clock()), small(), genesis())
                            {
                                fails.push(format!("op {n}: create after removal: {e}"));
                            }
                        }
                        Err(e) => fails.push(format!(
                            "op {n}: rebuild refused although seq {acked} was acknowledged: {e}"
                        )),
                    }
                }
                Err(e) => fails.push(format!("op {n}: create after a crashed create: {e}")),
            }
        }
        drop(lk);
        let _ = fs::remove_dir_all(&d);
        n += 1;
    }
    eprintln!(
        "start path: {} crash points; next start: {opened_after} open, {new_after} new campaign (all with at most record 1 acknowledged); {} failures",
        n - 1,
        fails.len()
    );
    for f in fails.iter().take(10) {
        eprintln!("  {f}");
    }
    assert!(fails.is_empty());
}
