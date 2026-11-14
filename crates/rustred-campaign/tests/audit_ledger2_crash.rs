//! Re-audit of the ledger (lane audit-ledger2, independent): crash-point enumeration at byte granularity
//! (SIGKILL model and power-loss model), truncation sweep and bit-flip sweep over the last two segments.
//!
//! Default runs use a stride (debug build); the full sweeps:
//! `RR_A2_STRIDE=1 cargo test -p rustred-campaign --release --test audit_ledger2_crash -- --nocapture`
mod ledger_support;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ledger_support::*;
use rustred_campaign::clock::{Clock, ManualClock, SystemClock};
use rustred_campaign::ledger::{
    self, Event, FileOps, Genesis, Ledger, LedgerConfig, LedgerError, OpenOptions, Record,
    SegmentFile,
};
use rustred_campaign::lock::{self, CampaignLock};
use rustred_campaign::state::EvidenceLevel;

// ------------------------------------------------------------------------------------------------------------
// helpers
// ------------------------------------------------------------------------------------------------------------

fn stride(default: u64) -> u64 {
    std::env::var("RR_A2_STRIDE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
        .max(1)
}

fn root(tag: &str) -> PathBuf {
    scratch(&format!("a2-{tag}"))
}

fn take_lock(cdir: &Path) -> CampaignLock {
    lock::acquire(cdir, &SystemClock::new(), &["audit-ledger2".into()])
        .unwrap()
        .0
}

fn cfg_small() -> LedgerConfig {
    LedgerConfig {
        rotate_bytes: 4096,
        ..LedgerConfig::default()
    }
}

fn wl_clock() -> Arc<dyn Clock> {
    Arc::new(ManualClock::at(T0).with_tick(250))
}

fn checker_clock() -> Arc<dyn Clock> {
    Arc::new(ManualClock::at("2030-01-01T00:00:00Z").with_tick(1))
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        let q = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &q);
        } else {
            fs::copy(&p, &q).unwrap();
            let mut perm = fs::metadata(&q).unwrap().permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            fs::set_permissions(&q, perm).unwrap();
        }
    }
}

/// Copies only the ledger directory of `src` into a fresh campaign directory.
fn clone_cdir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    copy_dir(&ledger::ledger_dir(src), &ledger::ledger_dir(dst));
}

/// Records written by the crashed workload (records the checker's own open appended carry its 2030 clock).
fn workload_records(cdir: &Path) -> Vec<Record> {
    let mut v = ledger::read_records(cdir).unwrap();
    while v.last().is_some_and(|r| r.utc.starts_with("2030")) {
        v.pop();
    }
    v
}

// ------------------------------------------------------------------------------------------------------------
// crash file operations: a global event budget; a write consumes one event per byte (segment appends) or a few
// events (other writes); every other operation one event. When the budget is exhausted the process "dies":
// the current write is left partial and NOTHING later reaches the disk (no truncation, rename, removal...).
// ------------------------------------------------------------------------------------------------------------

struct Crash {
    left: AtomicI64,
    dead: AtomicBool,
    used: AtomicU64,
    /// Content of every tracked file at its last successful fsync (power-loss model).
    synced: Mutex<BTreeMap<PathBuf, Vec<u8>>>,
}

fn killed() -> io::Error {
    io::Error::other("simulated SIGKILL")
}

impl Crash {
    fn new(limit: Option<u64>) -> Arc<Crash> {
        Arc::new(Crash {
            left: AtomicI64::new(limit.map(|l| l as i64).unwrap_or(i64::MAX)),
            dead: AtomicBool::new(false),
            used: AtomicU64::new(0),
            synced: Mutex::new(BTreeMap::new()),
        })
    }
    fn is_dead(&self) -> bool {
        self.dead.load(Ordering::SeqCst)
    }
    /// Takes up to `n` events; returns how many were granted (fewer = death).
    fn take(&self, n: u64) -> u64 {
        if self.is_dead() {
            return 0;
        }
        let left = self.left.load(Ordering::SeqCst).max(0) as u64;
        let k = left.min(n);
        self.left.fetch_sub(k as i64, Ordering::SeqCst);
        self.used.fetch_add(k, Ordering::SeqCst);
        if k < n {
            self.dead.store(true, Ordering::SeqCst);
        }
        k
    }
    fn op(&self) -> io::Result<()> {
        if self.take(1) == 1 {
            Ok(())
        } else {
            Err(killed())
        }
    }
    /// Bytes of an `n`-byte write that reach the file: byte granularity for segments, 4 steps otherwise.
    fn write_len(&self, n: usize, bytewise: bool) -> usize {
        if bytewise {
            return self.take(n as u64) as usize;
        }
        let w = 4u64;
        let k = self.take(w);
        if k == w {
            n
        } else {
            (k as usize * n) / w as usize
        }
    }
    fn note_synced(&self, path: &Path) {
        if let Ok(b) = fs::read(path) {
            self.synced.lock().unwrap().insert(path.to_path_buf(), b);
        }
    }
}

struct CFile {
    file: fs::File,
    len: u64,
    path: PathBuf,
    bytewise: bool,
    crash: Arc<Crash>,
}

impl SegmentFile for CFile {
    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        let k = self.crash.write_len(bytes.len(), self.bytewise);
        self.file.write_all_at(&bytes[..k], self.len)?;
        self.len += k as u64;
        if k < bytes.len() {
            return Err(killed());
        }
        Ok(())
    }
    fn sync(&mut self) -> io::Result<()> {
        self.crash.op()?;
        self.crash.note_synced(&self.path);
        Ok(())
    }
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        self.crash.op()?;
        self.file.set_len(len)?;
        self.len = len;
        self.crash.note_synced(&self.path);
        Ok(())
    }
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
        let k = self.crash.write_len(bytes.len(), self.bytewise);
        self.file.write_all_at(&bytes[..k], offset)?;
        self.len = self.len.max(offset + k as u64);
        if k < bytes.len() {
            return Err(killed());
        }
        Ok(())
    }
}

struct CrashOps(Arc<Crash>);

impl CrashOps {
    fn open_file(&self, path: &Path, bytewise: bool) -> io::Result<Box<dyn SegmentFile>> {
        self.0.op()?;
        let existed = path.exists();
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let len = file.metadata()?.len();
        if !existed {
            self.0.note_synced(path);
        }
        Ok(Box::new(CFile {
            file,
            len,
            path: path.to_path_buf(),
            bytewise,
            crash: self.0.clone(),
        }))
    }
}

impl FileOps for CrashOps {
    fn open_segment(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
        self.open_file(path, true)
    }
    fn open_mark(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
        self.open_file(path, false)
    }
    fn write_new_synced(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.0.op()?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        let k = self.0.write_len(bytes.len(), false);
        io::Write::write_all(&mut f, &bytes[..k])?;
        if k < bytes.len() {
            return Err(killed());
        }
        self.0.op()?; // its fsync
        self.0.note_synced(path);
        Ok(())
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.0.op()?;
        fs::rename(from, to)?;
        let mut s = self.0.synced.lock().unwrap();
        if let Some(b) = s.remove(from) {
            s.insert(to.to_path_buf(), b);
        }
        Ok(())
    }
    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        self.0.op()
    }
    fn remove(&self, path: &Path) -> io::Result<()> {
        self.0.op()?;
        fs::remove_file(path)
    }
}

// ------------------------------------------------------------------------------------------------------------
// the workload: two sessions; session 1 ends with a torn line written by hand (a kill inside a write), session
// 2 opens (torn-tail recovery under crash) and continues. Mix of sync / lazy records, single and group appends,
// explicit sync() calls, rotations every 4 KiB.
// ------------------------------------------------------------------------------------------------------------

#[allow(clippy::large_enum_variant)]
enum Step {
    One(Event),
    Group(Vec<Event>),
    Sync,
}

fn workload(items: usize) -> (Vec<Step>, Vec<Step>) {
    let node = "N1";
    let mut s1 = vec![Step::Group(setup_events(node))];
    let mut s2 = Vec::new();
    for k in 0..items {
        let life = item_life(node, k, (k % 3) as u8, EvidenceLevel::GeometryAccepted, 10);
        let dst = if k < items / 2 { &mut s1 } else { &mut s2 };
        match k % 3 {
            0 => dst.extend(life.into_iter().map(Step::One)),
            1 => dst.push(Step::Group(life)),
            _ => {
                dst.extend(life.into_iter().map(Step::One));
                dst.push(Step::One(Event::Note {
                    text: format!("lazy note {k}"),
                }));
                dst.push(Step::Sync);
            }
        }
    }
    // a lazy tail beyond the last commit mark
    for k in 0..2 {
        s2.push(Step::One(Event::Note {
            text: format!("lazy tail {k}"),
        }));
    }
    (s1, s2)
}

/// The torn line session 1 leaves behind (a record cut in the middle).
const TORN: &[u8] = b"{\"schema\":\"rustred.campaign.ledger.v1\",\"seq\":999,\"utc\":\"2026-1";

struct RunOut {
    /// Highest seq acknowledged (an append / sync that returned Ok before the death).
    acked: u64,
    /// Highest seq acknowledged as durable (Ok of a batch with a Sync record, or of sync()).
    durable: u64,
    crashed: bool,
}

fn run_steps(l: &mut Ledger, steps: &[Step], crash: &Crash, out: &mut RunOut) -> bool {
    for s in steps {
        let r = match s {
            Step::One(e) => l
                .append(e.clone())
                .map(|q| (q, e.durability() == ledger::Durability::Sync)),
            Step::Group(es) => l.append_all(es.clone()).map(|q| {
                (
                    *q.last().unwrap(),
                    es.iter()
                        .any(|e| e.durability() == ledger::Durability::Sync),
                )
            }),
            Step::Sync => l.sync().map(|_| (l.seq(), true)),
        };
        if crash.is_dead() {
            out.crashed = true;
            return false;
        }
        let (q, durable) =
            r.unwrap_or_else(|e| panic!("workload step failed without a crash: {e}"));
        out.acked = out.acked.max(q);
        if durable {
            out.durable = out.durable.max(q);
        }
    }
    true
}

/// Runs the workload in `cdir` with `limit` events (None: unlimited).
fn run_workload(cdir: &Path, items: usize, limit: Option<u64>) -> (RunOut, Arc<Crash>) {
    let crash = Crash::new(limit);
    let ops: Arc<dyn FileOps> = Arc::new(CrashOps(crash.clone()));
    let clk = wl_clock();
    let lk = take_lock(cdir);
    let (s1, s2) = workload(items);
    let mut out = RunOut {
        acked: 0,
        durable: 0,
        crashed: false,
    };
    let g = Genesis::new("crash-campaign", &digest("spec"), 100);
    let mut l = match Ledger::create_with(cdir, &lk, clk.clone(), cfg_small(), ops.clone(), g) {
        Ok(l) => l,
        Err(e) => {
            assert!(crash.is_dead(), "create failed without a crash: {e}");
            out.crashed = true;
            return (out, crash);
        }
    };
    if crash.is_dead() {
        out.crashed = true;
        return (out, crash);
    }
    out.acked = 1;
    out.durable = 1;
    if !run_steps(&mut l, &s1, &crash, &mut out) {
        return (out, crash);
    }
    // session 1 ends with a kill inside a write: a torn line beyond the last mark
    let seg = ledger::ledger_dir(cdir).join(&l.manifest().active.file);
    drop(l);
    {
        let f = fs::OpenOptions::new().append(true).open(&seg).unwrap();
        f.write_all_at(TORN, f.metadata().unwrap().len()).unwrap();
    }
    let mut l = match Ledger::open_with(cdir, &lk, clk, cfg_small(), ops, OpenOptions::default()) {
        Ok((l, _)) => l,
        Err(e) => {
            assert!(
                crash.is_dead(),
                "session-2 open failed without a crash: {e}"
            );
            out.crashed = true;
            return (out, crash);
        }
    };
    if crash.is_dead() {
        out.crashed = true;
        return (out, crash);
    }
    out.acked = l.seq();
    out.durable = l.seq();
    run_steps(&mut l, &s2, &crash, &mut out);
    (out, crash)
}

/// Opens after a crash with the real operations and checks the invariants. Returns a short verdict.
fn check_reopen(cdir: &Path, full: &[Record], out: &RunOut, need: u64) -> Result<usize, String> {
    let manifest = ledger::ledger_dir(cdir).join(ledger::MANIFEST_FILE);
    if !manifest.exists() {
        // no ledger yet: only allowed if nothing was acknowledged, and create must work again
        if out.acked > 0 && need > 0 {
            return Err(format!("no manifest but seq {need} acknowledged"));
        }
        let lk = take_lock(cdir);
        let g = Genesis::new("crash-campaign", &digest("spec"), 100);
        Ledger::create(cdir, &lk, checker_clock(), cfg_small(), g)
            .map_err(|e| format!("create after a crashed create: {e}"))?;
        return Ok(0);
    }
    let lk = take_lock(cdir);
    let (mut l, _rep) = Ledger::open_with(
        cdir,
        &lk,
        checker_clock(),
        cfg_small(),
        Arc::new(ledger::RealOps),
        OpenOptions::default(),
    )
    .map_err(|e| format!("reopen refused: {e}"))?;
    let got = workload_records(cdir);
    if got.len() > full.len() {
        return Err(format!(
            "{} records, more than the full run's {}",
            got.len(),
            full.len()
        ));
    }
    for (i, r) in got.iter().enumerate() {
        if *r != full[i] {
            return Err(format!(
                "record {} differs from the full run (kind {})",
                i + 1,
                r.event.kind()
            ));
        }
    }
    if (got.len() as u64) < need {
        return Err(format!(
            "only {} records, but seq {need} was acknowledged",
            got.len()
        ));
    }
    // reduced state of the handle = full replay of the raw segments; snapshot consistent
    let chain =
        ledger::verify_chain(cdir).map_err(|e| format!("verify_chain after reopen: {e}"))?;
    if chain.state != *l.state() {
        return Err("handle state differs from a full replay".into());
    }
    if chain.snapshot_matches == Some(false) {
        return Err("snapshot differs from the replayed state".into());
    }
    // the ledger continues and reopens again identically
    l.append(Event::Note {
        text: "after crash".into(),
    })
    .map_err(|e| format!("append after reopen: {e}"))?;
    let st = l.state().clone();
    drop(l);
    let (l2, _) = Ledger::open_with(
        cdir,
        &lk,
        checker_clock(),
        cfg_small(),
        Arc::new(ledger::RealOps),
        OpenOptions::default(),
    )
    .map_err(|e| format!("second reopen: {e}"))?;
    if *l2.state() != st {
        return Err("second reopen state differs".into());
    }
    Ok(got.len())
}

/// Power-loss transform: every tracked file falls back to its content at its last fsync; `nul` keeps the size
/// and fills the lost bytes with NUL (ext4-style), else the lost bytes are cut off.
fn power_loss(crash: &Crash, nul: bool) {
    let synced = crash.synced.lock().unwrap().clone();
    for (p, keep) in synced {
        let Ok(cur) = fs::read(&p) else { continue };
        let is_segment = p
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("segment-"));
        let mut v = keep.clone();
        if is_segment && nul && cur.len() > keep.len() {
            v.resize(cur.len(), 0);
        }
        if v != cur {
            fs::write(&p, &v).unwrap();
        }
    }
}

// ------------------------------------------------------------------------------------------------------------
// tests
// ------------------------------------------------------------------------------------------------------------

const ITEMS: usize = 8;

fn full_run(base: &Path) -> (Vec<Record>, u64, usize) {
    let d = base.join("full");
    fs::create_dir_all(&d).unwrap();
    let (out, crash) = run_workload(&d, ITEMS, None);
    assert!(!out.crashed);
    let recs = ledger::read_records(&d).unwrap();
    let segs = ledger::read_manifest(&ledger::ledger_dir(&d))
        .unwrap()
        .sealed
        .len()
        + 1;
    let used = crash.used.load(Ordering::SeqCst);
    (recs, used, segs)
}

#[test]
fn a2_crash_enumeration_sigkill_model() {
    let base = root("crash-kill");
    let _c = Cleanup(base.clone());
    let (full, events, segs) = full_run(&base);
    let step = stride(53);
    eprintln!(
        "full run: {} records, {segs} segments, {events} crash points (stride {step})",
        full.len()
    );
    assert!(segs >= 4, "the workload must span several segments");
    let mut fails = Vec::new();
    let mut n = 0u64;
    let mut outcomes: BTreeMap<String, u64> = BTreeMap::new();
    let mut e = 0;
    while e <= events {
        let d = base.join(format!("k{e}"));
        fs::create_dir_all(&d).unwrap();
        let (out, _crash) = run_workload(&d, ITEMS, Some(e));
        match check_reopen(&d, &full, &out, out.acked) {
            Ok(k) => {
                *outcomes
                    .entry(if k == full.len() {
                        "complete".into()
                    } else {
                        "prefix".into()
                    })
                    .or_default() += 1
            }
            Err(why) => fails.push(format!("crash point {e}: {why}")),
        }
        n += 1;
        let _ = fs::remove_dir_all(&d);
        e += if e < 2_000 { 1.max(step / 8) } else { step };
    }
    eprintln!(
        "SIGKILL model: {n} crash points checked, outcomes {outcomes:?}, {} failures",
        fails.len()
    );
    for f in fails.iter().take(20) {
        eprintln!("  {f}");
    }
    assert!(
        fails.is_empty(),
        "{} crash points violate the invariants",
        fails.len()
    );
}

#[test]
fn a2_crash_enumeration_power_loss_model() {
    let base = root("crash-power");
    let _c = Cleanup(base.clone());
    let (full, events, _) = full_run(&base);
    let step = stride(211);
    let mut fails = Vec::new();
    let mut n = 0u64;
    let mut e = 0;
    while e <= events {
        for nul in [false, true] {
            let d = base.join(format!("p{e}-{nul}"));
            fs::create_dir_all(&d).unwrap();
            let (out, crash) = run_workload(&d, ITEMS, Some(e));
            power_loss(&crash, nul);
            if let Err(why) = check_reopen(&d, &full, &out, out.durable) {
                fails.push(format!("crash point {e} (nul {nul}): {why}"));
            }
            n += 1;
            let _ = fs::remove_dir_all(&d);
        }
        e += step;
    }
    eprintln!(
        "power-loss model: {n} cases checked, {} failures",
        fails.len()
    );
    for f in fails.iter().take(20) {
        eprintln!("  {f}");
    }
    assert!(
        fails.is_empty(),
        "{} power-loss cases violate the invariants",
        fails.len()
    );
}

/// Truncation of the active segment or of the last sealed segment at EVERY offset: the ledger reopens to the
/// committed prefix when the cut is beyond the commit mark, and refuses (typed Corrupt) otherwise; the
/// read-only paths agree with `open`.
#[test]
fn a2_truncation_sweep_last_two_segments() {
    let base = root("trunc");
    let _c = Cleanup(base.clone());
    let src = base.join("src");
    fs::create_dir_all(&src).unwrap();
    let (out, _) = run_workload(&src, ITEMS, None);
    assert!(!out.crashed);
    let full = ledger::read_records(&src).unwrap();
    let ldir = ledger::ledger_dir(&src);
    let m = ledger::read_manifest(&ldir).unwrap();
    let mark = ledger::read_commit_mark(&ldir).unwrap().unwrap();
    assert_eq!(mark.segment, m.active.index);
    let last_sealed = m.sealed.last().unwrap().clone();
    let step = stride(1);
    let mut stats: BTreeMap<String, u64> = BTreeMap::new();
    let mut silent_read_records = Vec::new();
    let mut fails = Vec::new();
    for (file, is_active) in [
        (m.active.file.clone(), true),
        (last_sealed.file.clone(), false),
    ] {
        let len = fs::metadata(ldir.join(&file)).unwrap().len();
        let mut p = 0;
        while p < len + u64::from(is_active) {
            if p == len && !is_active {
                break;
            }
            let d = base.join(format!("t-{is_active}-{p}"));
            clone_cdir(&src, &d);
            let f = fs::OpenOptions::new()
                .write(true)
                .open(ledger::ledger_dir(&d).join(&file))
                .unwrap();
            f.set_len(p).unwrap();
            drop(f);
            let ro = ledger::open_read_only(&d);
            let vc = ledger::verify_chain(&d);
            let rr = ledger::read_records(&d);
            let expect_ok = is_active && p >= mark.offset;
            let lk = take_lock(&d);
            let op = Ledger::open_with(
                &d,
                &lk,
                checker_clock(),
                cfg_small(),
                Arc::new(ledger::RealOps),
                OpenOptions::default(),
            );
            match (&op, expect_ok) {
                (Ok(_), true) => {
                    let got = workload_records(&d);
                    if got[..] != full[..got.len()] {
                        fails.push(format!("{file}@{p}: reopened records are not a prefix"));
                    }
                    *stats.entry("reopened".into()).or_default() += 1;
                }
                (Err(LedgerError::Corrupt { .. }), false) => {
                    *stats.entry("refused".into()).or_default() += 1;
                }
                (Err(e), true) => {
                    fails.push(format!("{file}@{p}: beyond the mark but refused: {e}"))
                }
                (Ok(_), false) => fails.push(format!(
                    "{file}@{p}: acknowledged data cut but open accepted"
                )),
                (Err(e), false) => {
                    fails.push(format!("{file}@{p}: refused with a non-Corrupt error: {e}"))
                }
            }
            if !expect_ok {
                if ro.is_ok() {
                    fails.push(format!(
                        "{file}@{p}: open_read_only accepted a cut before the mark"
                    ));
                }
                if vc.is_ok() {
                    fails.push(format!(
                        "{file}@{p}: verify_chain accepted a cut before the mark"
                    ));
                }
                if let Ok(r) = &rr {
                    silent_read_records.push((file.clone(), p, r.len()));
                }
            }
            drop(op);
            drop(lk);
            let _ = fs::remove_dir_all(&d);
            p += step;
        }
    }
    eprintln!(
        "truncation sweep: {stats:?}; read_records silently returned a shorter history for {} cuts before the mark",
        silent_read_records.len()
    );
    for s in silent_read_records.iter().take(5) {
        eprintln!(
            "  read_records({}@{}) -> Ok({} records of {})",
            s.0,
            s.1,
            s.2,
            full.len()
        );
    }
    for f in fails.iter().take(20) {
        eprintln!("  {f}");
    }
    assert!(
        fails.is_empty(),
        "{} truncation cases violate the invariants",
        fails.len()
    );
}

/// Same truncation, but checks only `read_records` (used by `supervisor::result::read_final_result` and the
/// recovery scan): a cut of acknowledged records at a line boundary must not be read as a valid shorter history.
#[test]
fn a2_read_records_accepts_cut_acknowledged_tail() {
    let base = root("rr-cut");
    let _c = Cleanup(base.clone());
    let src = base.join("src");
    fs::create_dir_all(&src).unwrap();
    let (out, _) = run_workload(&src, ITEMS, None);
    assert!(!out.crashed);
    let full = ledger::read_records(&src).unwrap();
    let ldir = ledger::ledger_dir(&src);
    let m = ledger::read_manifest(&ldir).unwrap();
    let mark = ledger::read_commit_mark(&ldir).unwrap().unwrap();
    // cut the last acknowledged (fsynced, marked) record at its line start
    let bytes = fs::read(ldir.join(&m.active.file)).unwrap();
    let end = mark.offset as usize; // end of the last acknowledged (fsynced, marked) record
    let cut = bytes[..end - 1]
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    assert!((cut as u64) < mark.offset);
    let d = base.join("cut");
    clone_cdir(&src, &d);
    let f = fs::OpenOptions::new()
        .write(true)
        .open(ledger::ledger_dir(&d).join(&m.active.file))
        .unwrap();
    f.set_len(cut as u64).unwrap();
    drop(f);
    assert!(matches!(
        ledger::open_read_only(&d),
        Err(LedgerError::Corrupt { .. })
    ));
    assert!(matches!(
        ledger::verify_chain(&d),
        Err(LedgerError::Corrupt { .. })
    ));
    // a bit flip inside the (complete, acknowledged) last line: also a silent drop?
    let d2 = base.join("flip");
    clone_cdir(&src, &d2);
    let p2 = ledger::ledger_dir(&d2).join(&m.active.file);
    let mut b2 = fs::read(&p2).unwrap();
    b2.truncate(end); // the unsynced lazy tail is gone (a legitimate crash outcome)
    b2[cut + 20] ^= 0x04;
    fs::write(&p2, &b2).unwrap();
    assert!(matches!(
        ledger::open_read_only(&d2),
        Err(LedgerError::Corrupt { .. })
    ));
    let r1 = ledger::read_records(&d);
    let r2 = ledger::read_records(&d2);
    eprintln!(
        "read_records after cutting the acknowledged last record: {:?}; after a bit flip in it: {:?} (full {})",
        r1.as_ref().map(|v| v.len()).map_err(|e| e.to_string()),
        r2.as_ref().map(|v| v.len()).map_err(|e| e.to_string()),
        full.len()
    );
    assert!(
        r1.is_err(),
        "read_records returned a shorter history after acknowledged records were cut"
    );
    assert!(
        r2.is_err(),
        "read_records silently dropped an altered acknowledged record"
    );
}

/// One flipped bit at every byte of the last two segments (and the commit mark / manifest): never a silently
/// altered or dropped acknowledged record through `open`.
#[test]
fn a2_bitflip_sweep_last_two_segments() {
    let base = root("flip");
    let _c = Cleanup(base.clone());
    let src = base.join("src");
    fs::create_dir_all(&src).unwrap();
    let (out, _) = run_workload(&src, ITEMS, None);
    assert!(!out.crashed);
    let full = ledger::read_records(&src).unwrap();
    let ldir = ledger::ledger_dir(&src);
    let m = ledger::read_manifest(&ldir).unwrap();
    let mut files = vec![m.active.file.clone(), m.sealed.last().unwrap().file.clone()];
    files.push(ledger::COMMIT_FILE.into());
    files.push(ledger::MANIFEST_FILE.into());
    let mark_seq = ledger::read_commit_mark(&ldir).unwrap().unwrap().seq;
    let step = stride(7);
    let mut stats: BTreeMap<String, u64> = BTreeMap::new();
    let mut fails = Vec::new();
    for file in &files {
        let len = fs::metadata(ldir.join(file)).unwrap().len() as usize;
        let mut p = 0;
        while p < len {
            let d = base.join(format!("f-{p}"));
            clone_cdir(&src, &d);
            let path = ledger::ledger_dir(&d).join(file);
            let mut b = fs::read(&path).unwrap();
            b[p] ^= 1 << (p % 8);
            fs::write(&path, &b).unwrap();
            let lk = take_lock(&d);
            let op = Ledger::open_with(
                &d,
                &lk,
                checker_clock(),
                cfg_small(),
                Arc::new(ledger::RealOps),
                OpenOptions::default(),
            );
            let key = match &op {
                Ok(_) => {
                    let got = workload_records(&d);
                    if (got.len() as u64) < mark_seq {
                        fails.push(format!(
                            "{file}@{p}: open dropped acknowledged records ({} of {})",
                            got.len(),
                            full.len()
                        ));
                    } else if got[..] != full[..got.len()] {
                        fails.push(format!("{file}@{p}: open accepted an altered record"));
                    }
                    if got.len() < full.len() {
                        format!("{file}: accepted, unsynced tail beyond the mark dropped as torn")
                    } else {
                        format!("{file}: accepted unchanged")
                    }
                }
                Err(LedgerError::Corrupt { .. }) => format!("{file}: refused (Corrupt)"),
                Err(e) => format!(
                    "{file}: refused ({})",
                    e.to_string().split(':').next().unwrap_or("")
                ),
            };
            *stats.entry(key).or_default() += 1;
            drop(op);
            drop(lk);
            let _ = fs::remove_dir_all(&d);
            p += step as usize;
        }
    }
    for (k, v) in &stats {
        eprintln!("  {k}: {v}");
    }
    for f in fails.iter().take(20) {
        eprintln!("  {f}");
    }
    assert!(
        fails.is_empty(),
        "{} bit flips violate the invariants",
        fails.len()
    );
}
