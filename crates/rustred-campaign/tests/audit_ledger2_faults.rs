//! Re-audit of the ledger (lane audit-ledger2, independent): errno fault injection at EVERY file operation
//! (ENOSPC with a short write, EROFS, EIO), hand-forged records, swapped / deleted / foreign segments, a copied
//! campaign directory, the two lock layers, concurrent readers during rotation, and (process tests, run with
//! LIGHT_PIDNS=1) concurrent appender processes and a real full / read-only tmpfs in a private mount namespace.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
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

fn clk() -> Arc<dyn Clock> {
    Arc::new(ManualClock::at(T0).with_tick(250))
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let q = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &q);
        } else {
            fs::copy(e.path(), &q).unwrap();
            let mut perm = fs::metadata(&q).unwrap().permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            fs::set_permissions(&q, perm).unwrap();
        }
    }
}

fn clone_cdir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    copy_dir(&ledger::ledger_dir(src), &ledger::ledger_dir(dst));
}

fn events(items: usize) -> Vec<Vec<Event>> {
    let mut v = vec![setup_events("N1")];
    for k in 0..items {
        let life = item_life("N1", k, 0, EvidenceLevel::GeometryAccepted, 10);
        if k % 2 == 0 {
            v.extend(life.into_iter().map(|e| vec![e]));
        } else {
            v.push(life);
        }
    }
    v.push(vec![Event::Note {
        text: "lazy tail".into(),
    }]);
    v
}

fn build(cdir: &Path, items: usize) -> Vec<Record> {
    let lk = take_lock(cdir);
    let g = Genesis::new("fault-campaign", &digest("spec"), 100);
    let mut l = Ledger::create(cdir, &lk, clk(), cfg_small(), g).unwrap();
    for b in events(items) {
        l.append_all(b).unwrap();
    }
    drop(l);
    ledger::read_records(cdir).unwrap()
}

fn reopen(cdir: &Path) -> Result<(Ledger, CampaignLock), LedgerError> {
    let lk = take_lock(cdir);
    let (l, _) = Ledger::open_with(
        cdir,
        &lk,
        Arc::new(ManualClock::at("2030-01-01T00:00:00Z").with_tick(1)),
        cfg_small(),
        Arc::new(ledger::RealOps),
        OpenOptions::default(),
    )?;
    Ok((l, lk))
}

// ------------------------------------------------------------------------------------------------------------
// errno fault injection at every operation
// ------------------------------------------------------------------------------------------------------------

struct Fault {
    n: AtomicI64,
    errno: i32,
    hit: AtomicBool,
    count: AtomicU64,
    what: Mutex<String>,
}

impl Fault {
    /// true = this operation fails.
    fn step(&self, what: &str) -> bool {
        self.count.fetch_add(1, Ordering::SeqCst);
        if self.n.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.hit.store(true, Ordering::SeqCst);
            *self.what.lock().unwrap() = what.to_string();
            return true;
        }
        false
    }
    fn err(&self) -> io::Error {
        io::Error::from_raw_os_error(self.errno)
    }
}

struct FFile {
    file: fs::File,
    len: u64,
    name: String,
    f: Arc<Fault>,
}

impl SegmentFile for FFile {
    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.f.step(&format!("append {}", self.name)) {
            // a real ENOSPC usually leaves a short write behind
            let k = bytes.len() / 2;
            self.file.write_all_at(&bytes[..k], self.len)?;
            self.len += k as u64;
            return Err(self.f.err());
        }
        self.file.write_all_at(bytes, self.len)?;
        self.len += bytes.len() as u64;
        Ok(())
    }
    fn sync(&mut self) -> io::Result<()> {
        if self.f.step(&format!("fsync {}", self.name)) {
            return Err(self.f.err());
        }
        self.file.sync_data()
    }
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        if self.f.step(&format!("truncate {}", self.name)) {
            return Err(self.f.err());
        }
        self.file.set_len(len)?;
        self.len = len;
        Ok(())
    }
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
        if self.f.step(&format!("write_at {}", self.name)) {
            let k = bytes.len() / 2;
            self.file.write_all_at(&bytes[..k], offset)?;
            return Err(self.f.err());
        }
        self.file.write_all_at(bytes, offset)?;
        self.len = self.len.max(offset + bytes.len() as u64);
        Ok(())
    }
}

struct FaultOps(Arc<Fault>);

fn short(p: &Path) -> String {
    let n = p.file_name().unwrap().to_string_lossy().into_owned();
    if n.starts_with("segment-") {
        "segment".into()
    } else if n.contains("rr-tmp") {
        "tmp".into()
    } else if n.starts_with("torn-") {
        "torn".into()
    } else if n.starts_with("snapshot-") {
        "snapshot".into()
    } else {
        n
    }
}

impl FaultOps {
    fn open(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
        if self.0.step(&format!("open {}", short(path))) {
            return Err(self.0.err());
        }
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let len = file.metadata()?.len();
        Ok(Box::new(FFile {
            file,
            len,
            name: short(path),
            f: self.0.clone(),
        }))
    }
}

impl FileOps for FaultOps {
    fn open_segment(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
        self.open(path)
    }
    fn open_mark(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
        self.open(path)
    }
    fn write_new_synced(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if self.0.step(&format!("write+fsync {}", short(path))) {
            let _ = fs::write(path, &bytes[..bytes.len() / 2]);
            return Err(self.0.err());
        }
        ledger::RealOps.write_new_synced(path, bytes)
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        if self.0.step(&format!("rename -> {}", short(to))) {
            return Err(self.0.err());
        }
        fs::rename(from, to)
    }
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        if self.0.step("fsync-dir") {
            return Err(self.0.err());
        }
        rustred_campaign::atomic::sync_dir(dir)
    }
    fn remove(&self, path: &Path) -> io::Result<()> {
        if self.0.step(&format!("remove {}", short(path))) {
            return Err(self.0.err());
        }
        fs::remove_file(path)
    }
}

/// Runs the workload with operation `n` failing with `errno`; then "frees space" and resumes: reopen with the
/// real operations and append the rest. Returns a verdict line or an error.
fn fault_case(
    base: &Path,
    full: &[Record],
    n: i64,
    errno: i32,
    stats: &mut BTreeMap<String, u64>,
) -> Result<bool, String> {
    let d = base.join(format!("e{errno}-n{n}"));
    fs::create_dir_all(&d).unwrap();
    let f = Arc::new(Fault {
        n: AtomicI64::new(n),
        errno,
        hit: AtomicBool::new(false),
        count: AtomicU64::new(0),
        what: Mutex::new(String::new()),
    });
    let ops: Arc<dyn FileOps> = Arc::new(FaultOps(f.clone()));
    let lk = take_lock(&d);
    let g = Genesis::new("fault-campaign", &digest("spec"), 100);
    let mut acked = 0u64;
    let mut first_err: Option<String> = None;
    let mut poisoned_after = None;
    match Ledger::create_with(&d, &lk, clk(), cfg_small(), ops, g) {
        Ok(mut l) => {
            acked = 1;
            for b in events(4) {
                match l.append_all(b) {
                    Ok(q) => acked = *q.last().unwrap(),
                    Err(e) => {
                        first_err = Some(classify(&e, errno)?);
                        // every later call is refused (poisoned) or the error was a refusal
                        poisoned_after = Some(matches!(
                            l.append(Event::Note {
                                text: "after".into()
                            }),
                            Err(LedgerError::Poisoned { .. })
                        ));
                        break;
                    }
                }
            }
            if first_err.is_none() && f.hit.load(Ordering::SeqCst) {
                // a non-fatal failure (commit mark / rotation): a warning must say so
                let w = l.warnings().len();
                *stats
                    .entry(format!(
                        "ok with {} warning(s): {}",
                        w.min(1),
                        f.what.lock().unwrap()
                    ))
                    .or_default() += 1;
                if w == 0 && !f.what.lock().unwrap().starts_with("remove") {
                    return Err(format!(
                        "op {n} ({}) failed silently: no error, no warning",
                        f.what.lock().unwrap()
                    ));
                }
            }
        }
        Err(e) => {
            first_err = Some(classify(&e, errno)?);
        }
    }
    drop(lk);
    if !f.hit.load(Ordering::SeqCst) {
        return Ok(false); // beyond the last operation
    }
    if let Some(c) = &first_err {
        *stats
            .entry(format!("{c} at {}", f.what.lock().unwrap()))
            .or_default() += 1;
        if poisoned_after == Some(false) {
            return Err(format!(
                "op {n} ({}): handle not poisoned after {c}",
                f.what.lock().unwrap()
            ));
        }
    }
    // resume after freeing space
    if !ledger::ledger_dir(&d).join(ledger::MANIFEST_FILE).exists() {
        // procfix7 round 5 (gate9 G9-B1): create works again only into an absent or EMPTY ledger directory; the
        // residue of a failed create is REFUSED (ManifestMissing, nothing deleted) and recovered by rebuild-manifest,
        // or, without a complete record 1 (rebuild refuses), removed by the operator (here: the test) first
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
        let lk = take_lock(&d);
        let g = || Genesis::new("fault-campaign", &digest("spec"), 100);
        match Ledger::create(&d, &lk, clk(), cfg_small(), g()) {
            Ok(_) if before.is_empty() => {}
            Ok(_) => return Err(format!("op {n}: create over the residue {before:?}")),
            Err(rustred_campaign::ledger::LedgerError::ManifestMissing { .. }) => {
                if names() != before {
                    return Err(format!("op {n}: a refused create changed the residue"));
                }
                if ledger::rebuild_manifest(&d, &lk).is_err() {
                    let _ = fs::remove_dir_all(&ldir); // the operator's removal by hand
                    Ledger::create(&d, &lk, clk(), cfg_small(), g())
                        .map_err(|e| format!("op {n}: create after the removal: {e}"))?;
                }
            }
            Err(e) => return Err(format!("op {n}: create after a failed create: {e}")),
        }
        drop(lk);
        let _ = fs::remove_dir_all(&d);
        return Ok(true);
    }
    let (l, _lk) = reopen(&d)
        .map_err(|e| format!("op {n} ({}): reopen refused: {e}", f.what.lock().unwrap()))?;
    let mut got = ledger::read_records(&d).unwrap();
    while got.last().is_some_and(|r| r.utc.starts_with("2030")) {
        got.pop();
    }
    if (got.len() as u64) < acked {
        return Err(format!(
            "op {n}: {} records after reopen, {acked} acknowledged",
            got.len()
        ));
    }
    // same decisions (U-1): the event contents in order, ignoring seq / utc / prev and the position of the
    // informational segment_opened records (a failed rotation is retried later)
    let ev = |v: &[Record]| -> Vec<Event> {
        v.iter()
            .filter(|r| r.event.kind() != "segment_opened")
            .map(|r| r.event.clone())
            .collect()
    };
    let (g, fl) = (ev(&got), ev(full));
    if g.len() > fl.len() || g[..] != fl[..g.len()] {
        let i = g
            .iter()
            .zip(fl.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(g.len().min(fl.len()));
        return Err(format!(
            "op {n} ({}): event {i} differs from the full run",
            f.what.lock().unwrap()
        ));
    }
    let chain = ledger::verify_chain(&d).map_err(|e| format!("op {n}: verify: {e}"))?;
    if chain.state != *l.state() {
        return Err(format!("op {n}: state differs from replay"));
    }
    drop(l);
    let _ = fs::remove_dir_all(&d);
    Ok(true)
}

static TYPED_LOSS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn classify(e: &LedgerError, errno: i32) -> Result<String, String> {
    match (e, errno) {
        (LedgerError::NoSpace { .. }, libc::ENOSPC) => Ok("NoSpace".into()),
        (LedgerError::Io { source, .. }, x)
            if source.raw_os_error() == Some(x) && x != libc::ENOSPC =>
        {
            Ok(format!("Io(errno {x})"))
        }
        (LedgerError::Atomic(a), x) if x != libc::ENOSPC => Ok(format!("Atomic({a})")),
        (LedgerError::Poisoned { why }, x) if x != libc::ENOSPC => Ok(format!(
            "Poisoned (errno {x}: {})",
            why.split(':').next().unwrap_or("")
        )),
        (LedgerError::Poisoned { why }, libc::ENOSPC) => {
            TYPED_LOSS
                .lock()
                .unwrap()
                .push(why.chars().take(60).collect());
            Ok("Poisoned (ENOSPC cause; NOT typed NoSpace)".into())
        }
        (e, x) => Err(format!(
            "errno {x} surfaced as an untyped / wrong error: {e:?}"
        )),
    }
}

#[test]
fn a2_errno_fault_at_every_operation() {
    let base = scratch("a2-faults");
    let _c = Cleanup(base.clone());
    let src = base.join("full");
    fs::create_dir_all(&src).unwrap();
    let full = {
        let lk = take_lock(&src);
        let g = Genesis::new("fault-campaign", &digest("spec"), 100);
        let mut l = Ledger::create(&src, &lk, clk(), cfg_small(), g).unwrap();
        for b in events(4) {
            l.append_all(b).unwrap();
        }
        drop(l);
        ledger::read_records(&src).unwrap()
    };
    let mut fails = Vec::new();
    let mut n_cases = 0;
    for errno in [libc::ENOSPC, libc::EROFS, libc::EIO] {
        let mut stats = BTreeMap::new();
        let mut n = 1;
        loop {
            match fault_case(&base, &full, n, errno, &mut stats) {
                Ok(true) => {}
                Ok(false) => break,
                Err(why) => fails.push(format!("errno {errno}: {why}")),
            }
            n_cases += 1;
            n += 1;
        }
        eprintln!("errno {errno}: {} operations faulted", n - 1);
        for (k, v) in &stats {
            eprintln!("    {k}: {v}");
        }
    }
    for f in fails.iter().take(25) {
        eprintln!("  FAIL {f}");
    }
    let loss = TYPED_LOSS.lock().unwrap().clone();
    eprintln!(
        "ERRNO SWEEP: {n_cases} cases, {} failures; {} ENOSPC faults surfaced as Poisoned instead of NoSpace (supervisor maps them to Internal, not DiskFull):",
        fails.len(),
        loss.len()
    );
    for l in &loss {
        eprintln!("    {l}");
    }
    assert!(fails.is_empty());
    assert!(
        loss.is_empty(),
        "disk full lost its typed error in {} cases",
        loss.len()
    );
}

// ------------------------------------------------------------------------------------------------------------
// forged records, swapped / deleted / foreign segments, copied directory
// ------------------------------------------------------------------------------------------------------------

fn active_path(cdir: &Path) -> PathBuf {
    let m = ledger::read_manifest(&ledger::ledger_dir(cdir)).unwrap();
    ledger::ledger_dir(cdir).join(m.active.file)
}

fn lines(b: &[u8]) -> Vec<Vec<u8>> {
    b.split_inclusive(|&c| c == b'\n')
        .map(|l| l.to_vec())
        .collect()
}

fn outcome(cdir: &Path, full: &[Record]) -> String {
    match reopen(cdir) {
        Ok((_l, _lk)) => {
            let mut got = ledger::read_records(cdir).unwrap();
            while got.last().is_some_and(|r| r.utc.starts_with("2030")) {
                got.pop();
            }
            if got[..] == full[..got.len()] {
                format!("accepted prefix {}/{}", got.len(), full.len())
            } else {
                "ACCEPTED ALTERED HISTORY".into()
            }
        }
        Err(LedgerError::Corrupt { why, .. }) => {
            format!("Corrupt ({})", why.chars().take(60).collect::<String>())
        }
        Err(e) => format!("other error: {e}"),
    }
}

#[test]
fn a2_forged_and_foreign_records() {
    let base = scratch("a2-forge");
    let _c = Cleanup(base.clone());
    let src = base.join("src");
    fs::create_dir_all(&src).unwrap();
    build(&src, 6);
    {
        // a longer active segment: 3 sync records then a lazy one (same clock family as the build)
        let lk = take_lock(&src);
        let big = LedgerConfig {
            rotate_bytes: 1 << 20,
            ..LedgerConfig::default()
        };
        let (mut l, _) = Ledger::open(
            &src,
            &lk,
            Arc::new(ManualClock::at("2026-11-20T00:00:00Z").with_tick(250)),
            big,
        )
        .unwrap();
        for k in 0..3 {
            l.append(Event::Campaign {
                ev: rustred_campaign::state::CampaignEvent::Run,
                reason: format!("r{k}"),
            })
            .unwrap();
            l.append(Event::Intent {
                op: format!("op{k}"),
                action: "write".into(),
                target: "x".into(),
            })
            .unwrap();
        }
        l.append(Event::Note {
            text: "lazy".into(),
        })
        .unwrap();
    }
    let full = ledger::read_records(&src).unwrap();
    let ldir = ledger::ledger_dir(&src);
    let mark = ledger::read_commit_mark(&ldir).unwrap().unwrap();
    let mut table: Vec<(String, String, bool)> = Vec::new(); // (case, outcome, must refuse)
    let mut case = |name: &str, f: &dyn Fn(&Path), must_refuse: bool| {
        let d = base.join(name.replace(' ', "_"));
        clone_cdir(&src, &d);
        f(&d);
        let o = outcome(&d, &full);
        table.push((name.into(), o, must_refuse));
    };
    // (1) valid digest, broken chain: the last ACKNOWLEDGED record re-encoded with another prev
    case(
        "valid digest broken chain (acknowledged line)",
        &|d| {
            let p = active_path(d);
            let b = fs::read(&p).unwrap();
            let mut ls = lines(&b);
            let k = ls
                .iter()
                .scan(0usize, |s, l| {
                    *s += l.len();
                    Some(*s)
                })
                .position(|e| e as u64 == mark.offset)
                .unwrap();
            let (mut r, _) = ledger::decode_record(&ls[k][..ls[k].len() - 1]).unwrap();
            r.prev = digest("forged");
            let (line, _) = ledger::encode_record(&r);
            ls[k] = [line, b"\n".to_vec()].concat();
            fs::write(&p, ls.concat()).unwrap();
        },
        true,
    );
    // (2) valid chain position but bad digest (edited string, old h kept)
    case(
        "bad digest valid chain",
        &|d| {
            let p = active_path(d);
            let mut b = fs::read(&p).unwrap();
            let i = b.windows(4).position(|w| w == b"\"utc").unwrap() + 10;
            b[i] = if b[i] == b'1' { b'2' } else { b'1' };
            fs::write(&p, &b).unwrap();
        },
        true,
    );
    // (3) the lazy tail (beyond the mark) re-encoded with a broken chain
    case(
        "valid digest broken chain (unsynced tail line)",
        &|d| {
            let p = active_path(d);
            let b = fs::read(&p).unwrap();
            let mut ls = lines(&b);
            let k = ls.len() - 1;
            let (mut r, _) = ledger::decode_record(&ls[k][..ls[k].len() - 1]).unwrap();
            r.prev = digest("forged");
            let (line, _) = ledger::encode_record(&r);
            ls[k] = [line, b"\n".to_vec()].concat();
            fs::write(&p, ls.concat()).unwrap();
        },
        true,
    );
    // (4) a duplicated (replayed) last line
    case(
        "replayed last line",
        &|d| {
            let p = active_path(d);
            let b = fs::read(&p).unwrap();
            let ls = lines(&b);
            let mut v = ls.concat();
            v.extend_from_slice(ls.last().unwrap());
            fs::write(&p, v).unwrap();
        },
        true,
    );
    // (5) two lines swapped (reordered)
    case(
        "reordered lines",
        &|d| {
            let p = active_path(d);
            let b = fs::read(&p).unwrap();
            let mut ls = lines(&b);
            let n = ls.len();
            ls.swap(n - 2, n - 3);
            fs::write(&p, ls.concat()).unwrap();
        },
        true,
    );
    // (6) the whole unsynced tail rewritten as a valid chain continuation (content changed, re-hashed)
    case(
        "tail beyond the mark re-forged as a valid chain",
        &|d| {
            let p = active_path(d);
            let b = fs::read(&p).unwrap();
            let mut ls = lines(&b);
            let k = ls.len() - 1;
            let (mut r, _) = ledger::decode_record(&ls[k][..ls[k].len() - 1]).unwrap();
            r.event = Event::Note {
                text: "FORGED".into(),
            };
            let (line, _) = ledger::encode_record(&r);
            ls[k] = [line, b"\n".to_vec()].concat();
            fs::write(&p, ls.concat()).unwrap();
        },
        false,
    );
    // (7) sealed segments swapped
    case(
        "two sealed segments swapped",
        &|d| {
            let l = ledger::ledger_dir(d);
            let m = ledger::read_manifest(&l).unwrap();
            let (a, b) = (l.join(&m.sealed[1].file), l.join(&m.sealed[2].file));
            let t = l.join("tmp-swap");
            fs::rename(&a, &t).unwrap();
            fs::rename(&b, &a).unwrap();
            fs::rename(&t, &b).unwrap();
        },
        true,
    );
    // (8) a sealed segment deleted / the active segment deleted / the snapshot deleted
    case(
        "sealed segment deleted",
        &|d| {
            let l = ledger::ledger_dir(d);
            let m = ledger::read_manifest(&l).unwrap();
            fs::remove_file(l.join(&m.sealed[0].file)).unwrap();
        },
        true,
    );
    case(
        "active segment deleted",
        &|d| {
            fs::remove_file(active_path(d)).unwrap();
        },
        true,
    );
    case(
        "active segment emptied",
        &|d| {
            fs::write(active_path(d), b"").unwrap();
        },
        true,
    );
    case(
        "snapshot deleted (full replay fallback)",
        &|d| {
            let l = ledger::ledger_dir(d);
            let m = ledger::read_manifest(&l).unwrap();
            fs::remove_file(l.join(&m.snapshot.unwrap().file)).unwrap();
        },
        false,
    );
    case(
        "COMMIT deleted",
        &|d| {
            fs::remove_file(ledger::ledger_dir(d).join(ledger::COMMIT_FILE)).unwrap();
        },
        true,
    );
    case(
        "older sealed segment content placed in the active segment",
        &|d| {
            let l = ledger::ledger_dir(d);
            let m = ledger::read_manifest(&l).unwrap();
            fs::copy(l.join(&m.sealed.last().unwrap().file), active_path(d)).unwrap();
        },
        true,
    );
    case(
        "MANIFEST rolled back to before the last rotation",
        &|d| {
            let l = ledger::ledger_dir(d);
            let mut m = ledger::read_manifest(&l).unwrap();
            let last = m.sealed.pop().unwrap();
            m.active = ledger::ActiveSegment {
                index: last.index,
                file: last.file,
                first_seq: last.first_seq,
                prev: m
                    .sealed
                    .last()
                    .map(|s| s.last_hash.clone())
                    .unwrap_or(ledger::GENESIS.into()),
            };
            m.snapshot = None;
            fs::write(
                l.join(ledger::MANIFEST_FILE),
                serde_json::to_vec(&m).unwrap(),
            )
            .unwrap();
        },
        true,
    );
    // (9) foreign campaign: the active segment of a ledger with another campaign id (same layout)
    let other = base.join("other");
    fs::create_dir_all(&other).unwrap();
    {
        let lk = take_lock(&other);
        let mut l = Ledger::create(
            &other,
            &lk,
            clk(),
            cfg_small(),
            Genesis::new("OTHER-campaign", &digest("spec"), 100),
        )
        .unwrap();
        for b in events(6) {
            l.append_all(b).unwrap();
        }
    }
    let other_active = active_path(&other);
    case(
        "active segment of a foreign campaign",
        &|d| {
            fs::copy(&other_active, active_path(d)).unwrap();
        },
        true,
    );
    // (10) copied campaign directory that diverged: its active segment spliced back
    let copy = base.join("copy");
    clone_cdir(&src, &copy);
    {
        let (mut l, _lk) = reopen(&copy).unwrap();
        l.append(Event::Note {
            text: "only in the copy".into(),
        })
        .unwrap();
        l.sync().unwrap();
    }
    {
        let (mut l, _lk) = reopen(&src).unwrap();
        l.append(Event::Note {
            text: "only in the original".into(),
        })
        .unwrap();
        l.sync().unwrap();
    }
    let full2 = ledger::read_records(&src).unwrap();
    let copy_active = active_path(&copy);
    let d = base.join("splice");
    clone_cdir(&src, &d);
    fs::copy(&copy_active, active_path(&d)).unwrap();
    table.push((
        "diverged copy's active segment spliced in".into(),
        outcome(&d, &full2),
        true,
    ));
    let mut bad = 0;
    for (name, o, must) in &table {
        let refused = !o.starts_with("accepted") && !o.starts_with("ACCEPTED");
        let flag = if *must && !refused || o.starts_with("ACCEPTED") && *must {
            bad += 1;
            "  <-- FINDING"
        } else {
            ""
        };
        eprintln!("  {name}: {o}{flag}");
    }
    assert_eq!(bad, 0);
}

// ------------------------------------------------------------------------------------------------------------
// lock layers (in-process, no children)
// ------------------------------------------------------------------------------------------------------------

#[test]
fn a2_lock_layers() {
    let base = scratch("a2-lock");
    let _c = Cleanup(base.clone());
    let a = base.join("A");
    let b = base.join("B");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    build(&a, 1);
    build(&b, 1);
    // a lock of another directory
    let lb = take_lock(&b);
    assert!(matches!(
        Ledger::open(&a, &lb, clk(), cfg_small()),
        Err(LedgerError::WrongLock { .. })
    ));
    drop(lb);
    // LOCK deleted while A's handle lives: a second lock is possible, but the ledger directory flock refuses
    let la = take_lock(&a);
    let (mut h1, _) = Ledger::open(&a, &la, clk(), cfg_small()).unwrap();
    fs::remove_file(a.join(lock::LOCK_FILE)).unwrap();
    let la2 = lock::acquire(&a, &SystemClock::new(), &["second".into()]);
    let la2 = la2
        .expect("a fresh LOCK inode can be taken after the old one was deleted")
        .0;
    let second = Ledger::open(&a, &la2, clk(), cfg_small());
    eprintln!(
        "second write handle with a fresh lock while the first lives: {:?}",
        second.as_ref().map(|_| ()).map_err(|e| e.to_string())
    );
    assert!(matches!(second, Err(LedgerError::Busy { .. })));
    h1.append(Event::Note {
        text: "first still writes".into(),
    })
    .unwrap();
    drop(h1);
    // the first owner cannot reopen with its now replaced lock
    assert!(matches!(
        Ledger::open(&a, &la, clk(), cfg_small()),
        Err(LedgerError::WrongLock { .. })
    ));
    let (_h2, _) = Ledger::open(&a, &la2, clk(), cfg_small()).unwrap();
    // read-only views never need the lock
    assert!(ledger::open_read_only(&a).is_ok());
}

// ------------------------------------------------------------------------------------------------------------
// concurrent readers during appends and rotations (threads; readers take no lock)
// ------------------------------------------------------------------------------------------------------------

#[test]
fn a2_readers_during_rotation() {
    let base = scratch("a2-readers");
    let _c = Cleanup(base.clone());
    let d = base.join("c");
    fs::create_dir_all(&d).unwrap();
    let lk = take_lock(&d);
    let keep: usize = std::env::var("RR_A2_KEEP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let cfg = LedgerConfig {
        rotate_bytes: 2048,
        keep_old_snapshots: keep,
        ..LedgerConfig::default()
    };
    let mut l = Ledger::create(
        &d,
        &lk,
        Arc::new(SystemClock::new()),
        cfg,
        Genesis::new("rd", &digest("s"), 9),
    )
    .unwrap();
    l.append_all(setup_events("N1")).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let errs: Arc<Mutex<BTreeMap<String, u64>>> = Arc::default();
    let reads = Arc::new(AtomicU64::new(0));
    let mut hs = Vec::new();
    for t in 0..3 {
        let (d, stop, errs, reads) = (d.clone(), stop.clone(), errs.clone(), reads.clone());
        hs.push(std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let r = if t == 2 {
                    ledger::verify_chain(&d).map(|_| ())
                } else {
                    ledger::open_read_only(&d).map(|_| ())
                };
                reads.fetch_add(1, Ordering::SeqCst);
                if let Err(e) = r {
                    let k = match &e {
                        LedgerError::Corrupt { why, .. } => {
                            format!("Corrupt: {}", why.chars().take(70).collect::<String>())
                        }
                        e => {
                            let s = e.to_string();
                            let file = s
                                .rsplit('/')
                                .next()
                                .unwrap_or("")
                                .chars()
                                .take(40)
                                .collect::<String>();
                            format!("{} .. {file}", s.split(" of ").next().unwrap_or(""))
                        }
                    };
                    *errs.lock().unwrap().entry(k).or_default() += 1;
                }
            }
        }));
    }
    for k in 0..400 {
        l.append_all(item_life("N1", k, 0, EvidenceLevel::GeometryAccepted, 1))
            .unwrap();
    }
    let segs = l.manifest().sealed.len();
    stop.store(true, Ordering::SeqCst);
    for h in hs {
        h.join().unwrap();
    }
    let errs = errs.lock().unwrap().clone();
    eprintln!(
        "READERS (keep_old_snapshots {keep}): {} reads during {segs} rotations; errors {errs:?}",
        reads.load(Ordering::SeqCst)
    );
    let corrupt: u64 = errs
        .iter()
        .filter(|(k, _)| k.starts_with("Corrupt"))
        .map(|(_, v)| v)
        .sum();
    let total: u64 = errs.values().sum();
    assert_eq!(corrupt, 0, "a reader saw a healthy ledger as corrupt");
    assert_eq!(total, 0, "a reader failed on a healthy ledger");
}

// ------------------------------------------------------------------------------------------------------------
// process tests (LIGHT_PIDNS=1): concurrent appender processes; real full / read-only tmpfs
// ------------------------------------------------------------------------------------------------------------

fn in_pidns() -> bool {
    // inside `unshare -Urpf --mount-proc` this binary is pid 1 of its namespace (or a descendant of it)
    fs::read_to_string("/proc/1/cmdline")
        .map(|c| c.contains("audit_ledger2_faults"))
        .unwrap_or(false)
}

/// Child body: appends `rounds` x 5 notes, taking and releasing the campaign lock each round.
#[test]
fn a2_child_appender() {
    let Ok(cdir) = std::env::var("RR_A2_APPENDER") else {
        return;
    };
    let cdir = PathBuf::from(cdir);
    let tag = std::env::var("RR_A2_TAG").unwrap();
    let mut done = 0;
    let mut busy: BTreeMap<String, u64> = BTreeMap::new();
    while done < 40 {
        match lock::acquire(&cdir, &SystemClock::new(), std::slice::from_ref(&tag)) {
            Ok((lk, _)) => {
                let (mut l, _) =
                    Ledger::open(&cdir, &lk, Arc::new(SystemClock::new()), cfg_small()).unwrap();
                let evs = (0..5)
                    .map(|i| Event::Note {
                        text: format!("{tag} r{done} n{i}"),
                    })
                    .collect();
                l.append_all(evs).unwrap();
                l.append(Event::Campaign {
                    ev: rustred_campaign::state::CampaignEvent::Run,
                    reason: tag.clone(),
                })
                .ok();
                done += 1;
            }
            Err(e) => {
                let k = format!("{e}")
                    .split(':')
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(50)
                    .collect::<String>();
                *busy.entry(k).or_default() += 1;
                std::thread::yield_now();
            }
        }
    }
    println!("CHILD {tag} rounds {done} refusals {busy:?}");
}

#[test]
fn a2_concurrent_appender_processes() {
    if !in_pidns() {
        eprintln!("skipped: run with LIGHT_PIDNS=1 (spawns children)");
        return;
    }
    let base = scratch("a2-procs");
    let _c = Cleanup(base.clone());
    let d = base.join("c");
    fs::create_dir_all(&d).unwrap();
    {
        let lk = take_lock(&d);
        let mut l = Ledger::create(
            &d,
            &lk,
            clk(),
            cfg_small(),
            Genesis::new("procs", &digest("s"), 9),
        )
        .unwrap();
        l.append_all(setup_events("N1")).unwrap();
    }
    let n0 = ledger::read_records(&d).unwrap().len();
    let exe = std::env::current_exe().unwrap();
    let mut kids = Vec::new();
    for k in 0..4 {
        kids.push(
            std::process::Command::new(&exe)
                .args([
                    "--exact",
                    "a2_child_appender",
                    "--nocapture",
                    "--test-threads",
                    "1",
                ])
                .env("RR_A2_APPENDER", &d)
                .env("RR_A2_TAG", format!("P{k}"))
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    for c in kids {
        let o = c.wait_with_output().unwrap();
        assert!(o.status.success(), "child failed");
        let s = String::from_utf8_lossy(&o.stdout);
        eprintln!(
            "  {}",
            s.lines().find(|l| l.contains("CHILD")).unwrap_or("?")
        );
    }
    let chain = ledger::verify_chain(&d).unwrap();
    let notes = ledger::read_records(&d)
        .unwrap()
        .iter()
        .filter(|r| matches!(&r.event, Event::Note { .. }))
        .count();
    eprintln!(
        "CONCURRENT APPENDERS: 4 processes x 40 rounds; {} records, {notes} notes, {} segments",
        chain.records, chain.segments
    );
    assert_eq!(notes, 4 * 40 * 5);
    assert!(chain.records as usize >= n0 + 4 * 40 * 5);
    // every round's 5 notes are contiguous (one group commit, never interleaved)
    let recs = ledger::read_records(&d).unwrap();
    let mut i = 0;
    while i < recs.len() {
        if let Event::Note { text } = &recs[i].event {
            let pre = text.rsplit_once(' ').unwrap().0.to_string();
            for j in 0..5 {
                let Event::Note { text: t } = &recs[i + j].event else {
                    panic!("interleaved group")
                };
                assert!(
                    t.starts_with(&pre),
                    "interleaved group at seq {}",
                    recs[i + j].seq
                );
            }
            i += 5;
        } else {
            i += 1;
        }
    }
}

/// Inner body (run by the outer test inside `unshare -Urm`): a 192 KiB tmpfs fills up; then it is remounted
/// read-only.
#[test]
fn a2_real_fs_inner() {
    let Ok(mnt) = std::env::var("RR_A2_MNT") else {
        return;
    };
    let mnt = PathBuf::from(mnt);
    let cs = |s: &str| std::ffi::CString::new(s).unwrap();
    let m = cs(mnt.to_str().unwrap());
    let r = unsafe {
        libc::mount(
            cs("tmpfs").as_ptr(),
            m.as_ptr(),
            cs("tmpfs").as_ptr(),
            0,
            cs("size=192k").as_ptr() as *const libc::c_void,
        )
    };
    assert_eq!(r, 0, "mount tmpfs: {}", io::Error::last_os_error());
    let d = mnt.join("c");
    fs::create_dir_all(&d).unwrap();
    let lk = take_lock(&d);
    let cfg = LedgerConfig {
        rotate_bytes: 16 << 10,
        ..LedgerConfig::default()
    };
    let mut l = Ledger::create(
        &d,
        &lk,
        clk(),
        cfg.clone(),
        Genesis::new("fs", &digest("s"), 9),
    )
    .unwrap();
    l.append_all(setup_events("N1")).unwrap();
    let mut acked = l.seq();
    let mut err = None;
    for k in 0..100_000 {
        match l.append_all(item_life("N1", k, 0, EvidenceLevel::GeometryAccepted, 1)) {
            Ok(q) => acked = *q.last().unwrap(),
            Err(e) => {
                err = Some(e);
                break;
            }
        }
    }
    let e = err.expect("the tmpfs never filled");
    println!(
        "FULL: first error after seq {acked}: {e}; warnings {:?}",
        l.warnings()
    );
    assert!(
        matches!(e, LedgerError::NoSpace { .. }),
        "disk full gave {e:?}"
    );
    assert!(matches!(
        l.append(Event::Note { text: "x".into() }),
        Err(LedgerError::Poisoned { .. })
    ));
    drop(l);
    // still full: reopen must not corrupt anything (it may fail with a typed error)
    let r = Ledger::open(&d, &lk, clk(), cfg.clone());
    println!(
        "FULL: reopen while full: {:?}",
        r.as_ref().map(|_| ()).map_err(|e| e.to_string())
    );
    drop(r);
    let ro = ledger::open_read_only(&d).unwrap();
    assert!(
        ro.seq >= acked,
        "acknowledged records lost: view at {} < {acked}",
        ro.seq
    );
    // free space (remove the snapshots kept besides the manifest's), reopen
    let fill = d.join("filler");
    let _ = fs::remove_file(&fill);
    let (l, rep) = match Ledger::open(&d, &lk, clk(), cfg.clone()) {
        Ok(x) => x,
        Err(e) => {
            // make room: drop every file not referenced by the manifest
            println!("FULL: reopen still refused ({e}); removing torn-*.bin forensics");
            for e in fs::read_dir(ledger::ledger_dir(&d)).unwrap().flatten() {
                if e.file_name().to_string_lossy().starts_with("torn-") {
                    fs::remove_file(e.path()).unwrap();
                }
            }
            Ledger::open(&d, &lk, clk(), cfg.clone()).unwrap()
        }
    };
    println!(
        "FULL: reopened at seq {} (recovery {:?})",
        l.seq(),
        rep.recovery.as_ref().map(|r| r.truncated_bytes)
    );
    assert!(l.seq() >= acked);
    let st = l.state().clone();
    drop(l);
    assert!(ledger::verify_chain(&d).unwrap().state == st);
    // read-only remount (needs no file open for writing: the lock is released first)
    drop(lk);
    let flags = libc::MS_REMOUNT | libc::MS_RDONLY;
    let r = unsafe {
        libc::mount(
            std::ptr::null(),
            m.as_ptr(),
            std::ptr::null(),
            flags,
            std::ptr::null(),
        )
    };
    assert_eq!(r, 0, "remount ro: {}", io::Error::last_os_error());
    let lr = lock::acquire(&d, &SystemClock::new(), &["ro".into()]);
    println!(
        "RO: lock::acquire on a read-only fs: {:?}",
        lr.as_ref().map(|_| ()).map_err(|e| e.to_string())
    );
    assert!(lr.is_err());
    let v = ledger::open_read_only(&d).unwrap();
    assert_eq!(v.state, st);
    println!("RO: open_read_only ok at seq {}", v.seq);
    println!("INNER OK");
}

#[test]
fn a2_real_full_and_readonly_fs() {
    if !in_pidns() {
        eprintln!("skipped: run with LIGHT_PIDNS=1 (spawns a child in a private mount namespace)");
        return;
    }
    let base = scratch("a2-realfs");
    let _c = Cleanup(base.clone());
    let mnt = base.join("mnt");
    fs::create_dir_all(&mnt).unwrap();
    let exe = std::env::current_exe().unwrap();
    let o = std::process::Command::new("unshare")
        .args(["-Urm", "--"])
        .arg(&exe)
        .args([
            "--exact",
            "a2_real_fs_inner",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env("RR_A2_MNT", &mnt)
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    for l in out
        .lines()
        .filter(|l| l.contains("FULL") || l.contains("RO:") || l.contains("INNER"))
    {
        eprintln!("  {l}");
    }
    eprintln!(
        "{}",
        String::from_utf8_lossy(&o.stderr)
            .lines()
            .filter(|l| l.contains("panicked") || l.contains("assert"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        o.status.success() && out.contains("INNER OK"),
        "inner test failed"
    );
}

// ------------------------------------------------------------------------------------------------------------
// a missing manifest, partial page loss beyond the mark
// ------------------------------------------------------------------------------------------------------------

/// The supervisor calls `Ledger::create` whenever `ledger/MANIFEST.json` is absent (supervisor/mod.rs, start);
/// `create` then deletes every `segment-*.jsonl`, snapshot and the commit mark as "leftovers of a crashed
/// create", even when they hold an acknowledged multi-segment history.
#[test]
fn a2_missing_manifest_create_wipes_history() {
    let base = scratch("a2-nomanifest");
    let _c = Cleanup(base.clone());
    let d = base.join("c");
    fs::create_dir_all(&d).unwrap();
    let full = build(&d, 6);
    let ldir = ledger::ledger_dir(&d);
    let segs_before = fs::read_dir(&ldir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("segment-"))
        .count();
    let mark = ledger::read_commit_mark(&ldir).unwrap().unwrap();
    fs::remove_file(ldir.join(ledger::MANIFEST_FILE)).unwrap();
    let lk = take_lock(&d);
    let r = Ledger::create(
        &d,
        &lk,
        clk(),
        cfg_small(),
        Genesis::new("fault-campaign", &digest("spec"), 100),
    );
    let segs_after = fs::read_dir(&ldir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("segment-"))
        .count();
    let recs_after = ledger::read_records(&d).map(|v| v.len()).unwrap_or(0);
    eprintln!(
        "manifest deleted; {} records ({segs_before} segments, commit mark at seq {}) on disk; create -> {:?}; afterwards {segs_after} segment(s), {recs_after} record(s)",
        full.len(),
        mark.seq,
        r.as_ref().map(|_| ()).map_err(|e| e.to_string())
    );
    assert!(
        r.is_err(),
        "create silently deleted an acknowledged history of {} records",
        full.len()
    );
}

/// Out-of-order write-back of UNSYNCED data (ext4-style power loss): the first unsynced page is lost (NUL) but a
/// later page with complete valid records survives, all beyond the commit mark. Everything there is
/// unacknowledged, yet `open` refuses (Corrupt: "valid records follow") and needs the operator override.
#[test]
fn a2_partial_page_loss_beyond_mark() {
    let base = scratch("a2-pageloss");
    let _c = Cleanup(base.clone());
    let d = base.join("c");
    fs::create_dir_all(&d).unwrap();
    {
        let lk = take_lock(&d);
        let big = LedgerConfig {
            rotate_bytes: 1 << 20,
            lazy_max_records: 10_000,
            lazy_max_ms: 1 << 40,
            ..LedgerConfig::default()
        };
        let mut l = Ledger::create(
            &d,
            &lk,
            Arc::new(ManualClock::at(T0)),
            big,
            Genesis::new("p", &digest("s"), 9),
        )
        .unwrap();
        l.append_all(setup_events("N1")).unwrap();
        for k in 0..40 {
            l.append(Event::Note {
                text: format!("lazy {k} {}", "x".repeat(200)),
            })
            .unwrap();
        }
    }
    let ldir = ledger::ledger_dir(&d);
    let mark = ledger::read_commit_mark(&ldir).unwrap().unwrap();
    let p = active_path(&d);
    let mut b = fs::read(&p).unwrap();
    let lost = mark.offset as usize..(mark.offset as usize + 4096).min(b.len());
    for x in &mut b[lost.clone()] {
        *x = 0;
    }
    fs::write(&p, &b).unwrap();
    let r = reopen(&d);
    eprintln!(
        "unsynced bytes {}..{} lost (NUL), later unsynced records intact: open -> {:?}",
        lost.start,
        lost.end,
        r.as_ref().map(|(l, _)| l.seq()).map_err(|e| e
            .to_string()
            .chars()
            .take(120)
            .collect::<String>())
    );
    assert!(
        r.is_ok(),
        "a power loss that damaged only unacknowledged bytes needs the operator override to resume"
    );
}
