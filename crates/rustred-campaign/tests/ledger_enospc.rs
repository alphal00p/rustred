//! Disk full: atomic writes (with /dev/full and a quota-like failing writer) and ledger appends fail with a
//! typed error, leave the old file / the ledger valid, and leave no stray temporary.
mod ledger_support;

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use ledger_support::*;
use rustred_campaign::atomic::{self, AtomicError, Mode, Staging, StagingWrite};
use rustred_campaign::ledger::{
    self, Event, FileOps, LedgerConfig, LedgerError, OpenOptions, SegmentFile,
};
use rustred_campaign::state::EvidenceLevel;
use std::os::unix::fs::FileExt;

/// A writer that accepts `budget` bytes, then fails like a full disk (ENOSPC) or a quota (EDQUOT).
struct Quota {
    inner: fs::File,
    budget: Arc<AtomicI64>,
    errno: i32,
}

impl Write for Quota {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let left = self.budget.load(Ordering::SeqCst);
        if left <= 0 {
            return Err(io::Error::from_raw_os_error(self.errno));
        }
        let n = (buf.len() as i64).min(left) as usize;
        let k = self.inner.write(&buf[..n])?;
        self.budget.fetch_sub(k as i64, Ordering::SeqCst);
        Ok(k)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl StagingWrite for Quota {
    fn sync(&mut self) -> io::Result<()> {
        self.inner.sync_all()
    }
}

struct QuotaStaging {
    budget: Arc<AtomicI64>,
    errno: i32,
}

impl Staging for QuotaStaging {
    fn create(&self, temp: &Path) -> io::Result<Box<dyn StagingWrite>> {
        let inner = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temp)?;
        Ok(Box::new(Quota {
            inner,
            budget: self.budget.clone(),
            errno: self.errno,
        }))
    }
}

/// The staging file exists in the target directory, but every byte goes to /dev/full.
struct DevFullStaging;

struct DevFull(fs::File);
impl Write for DevFull {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
impl StagingWrite for DevFull {
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Staging for DevFullStaging {
    fn create(&self, temp: &Path) -> io::Result<Box<dyn StagingWrite>> {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temp)?;
        Ok(Box::new(DevFull(
            fs::OpenOptions::new().write(true).open("/dev/full")?,
        )))
    }
}

fn entries(d: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn dev_full_is_classified_as_no_space() {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let e = f.write_all(b"x").unwrap_err();
    assert!(atomic::is_no_space(&e), "{e:?}");
    let d = scratch("devfull");
    let _c = Cleanup(d.clone());
    let p = d.join("progress.json");
    atomic::write_atomic(&p, b"old").unwrap();
    let err = atomic::write_atomic_with(&p, Mode::Replace, &DevFullStaging, |w| {
        w.write_all(&[7u8; 100_000])
    })
    .unwrap_err();
    assert!(matches!(err, AtomicError::NoSpace { .. }), "{err}");
    assert_eq!(fs::read(&p).unwrap(), b"old", "old file intact");
    assert_eq!(entries(&d), vec!["progress.json"], "no stray temporary");
}

#[test]
fn quota_like_failure_keeps_old_file_and_removes_temp() {
    for errno in [libc::ENOSPC, libc::EDQUOT] {
        let d = scratch("quota");
        let _c = Cleanup(d.clone());
        let p = d.join("campaign.json");
        atomic::write_atomic(&p, b"{\"old\":true}\n").unwrap();
        let staging = QuotaStaging {
            budget: Arc::new(AtomicI64::new(4096)),
            errno,
        };
        let err = atomic::write_atomic_with(&p, Mode::Replace, &staging, |w| {
            w.write_all(&vec![b'a'; 10_000])
        })
        .unwrap_err();
        assert!(err.is_no_space(), "{err}");
        assert_eq!(fs::read(&p).unwrap(), b"{\"old\":true}\n");
        assert_eq!(entries(&d), vec!["campaign.json"]);
        // within the budget the same call succeeds
        let staging = QuotaStaging {
            budget: Arc::new(AtomicI64::new(1 << 20)),
            errno,
        };
        atomic::write_atomic_with(&p, Mode::Replace, &staging, |w| w.write_all(b"new")).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"new");
        // no-clobber create under quota: typed error, nothing created
        let q = d.join("fresh.json");
        let staging = QuotaStaging {
            budget: Arc::new(AtomicI64::new(3)),
            errno,
        };
        assert!(
            atomic::write_atomic_with(&q, Mode::NoClobber, &staging, |w| w.write_all(b"too long"))
                .unwrap_err()
                .is_no_space()
        );
        assert!(!q.exists());
        assert_eq!(entries(&d), vec!["campaign.json"]);
    }
}

/// Segment files that fail once a shared byte budget is spent (partial writes included).
struct QuotaSegment {
    f: fs::File,
    len: u64,
    budget: Arc<AtomicI64>,
}

impl SegmentFile for QuotaSegment {
    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        let left = self.budget.load(Ordering::SeqCst);
        if (bytes.len() as i64) > left {
            // write what fits (a torn record), then fail like the kernel would
            let n = left.max(0) as usize;
            self.f.write_all_at(&bytes[..n], self.len)?;
            self.len += n as u64;
            self.budget.store(0, Ordering::SeqCst);
            return Err(io::Error::from_raw_os_error(libc::ENOSPC));
        }
        self.f.write_all_at(bytes, self.len)?;
        self.len += bytes.len() as u64;
        self.budget.fetch_sub(bytes.len() as i64, Ordering::SeqCst);
        Ok(())
    }
    fn sync(&mut self) -> io::Result<()> {
        self.f.sync_data()
    }
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        self.f.set_len(len)?;
        self.len = len;
        Ok(())
    }
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
        self.f.write_all_at(bytes, offset)?;
        self.len = self.len.max(offset + bytes.len() as u64);
        Ok(())
    }
}

struct QuotaOpener(Arc<AtomicI64>);
impl FileOps for QuotaOpener {
    fn open_segment(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
        let f = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let len = f.metadata()?.len();
        Ok(Box::new(QuotaSegment {
            f,
            len,
            budget: self.0.clone(),
        }))
    }
}

#[test]
fn ledger_append_on_full_disk_is_typed_and_leaves_a_valid_ledger() {
    let d = scratch("ledgerfull");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let budget = Arc::new(AtomicI64::new(i64::MAX));
    let mut l = new_ledger_with(
        &d,
        as_dyn(&clk),
        LedgerConfig::default(),
        Arc::new(QuotaOpener(budget.clone())),
    )
    .unwrap();
    l.append_all(setup_events("T162")).unwrap();
    l.append_all(item_life("T162", 0, 0, EvidenceLevel::IdentityTested, 10))
        .unwrap();
    let (seq, head) = (l.seq(), l.head().to_string());
    let seg = ledger::ledger_dir(&d).join(&l.manifest().active.file);
    let len = fs::metadata(&seg).unwrap().len();
    budget.store(100, Ordering::SeqCst); // the next record does not fit
    let err = l
        .append_all(item_life("T162", 1, 0, EvidenceLevel::IdentityTested, 10))
        .unwrap_err();
    assert!(matches!(err, LedgerError::NoSpace { .. }), "{err}");
    assert_eq!(
        fs::metadata(&seg).unwrap().len(),
        len,
        "partial bytes rolled back"
    );
    assert!(matches!(
        l.append(Event::Note { text: "x".into() }),
        Err(LedgerError::Poisoned { .. })
    ));
    drop(l);
    // space is freed: reopen, no recovery needed, same head
    budget.store(i64::MAX, Ordering::SeqCst);
    let (mut l2, rep) = open_ledger_with(
        &d,
        as_dyn(&clk),
        LedgerConfig::default(),
        Arc::new(QuotaOpener(budget.clone())),
        OpenOptions::default(),
    )
    .unwrap();
    assert_eq!(rep.recovery, None);
    assert_eq!((l2.seq(), l2.head()), (seq, head.as_str()));
    l2.append_all(item_life("T162", 1, 0, EvidenceLevel::IdentityTested, 10))
        .unwrap();
    assert!(ledger::verify_chain(&d).is_ok());
    // and even if the rollback had failed (torn bytes left), open recovers by truncation
    drop(l2);
    fs::OpenOptions::new()
        .append(true)
        .open(&seg)
        .unwrap()
        .write_all(b"{\"schema\":\"rustred.camp")
        .unwrap();
    let (_l3, rep) = open_ledger(&d, as_dyn(&clk), LedgerConfig::default()).unwrap();
    assert!(rep.recovery.is_some());
}

#[test]
fn rotation_on_full_disk_keeps_the_previous_manifest() {
    let d = scratch("rotfull");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let budget = Arc::new(AtomicI64::new(i64::MAX));
    let cfg = LedgerConfig {
        rotate_bytes: u64::MAX,
        ..LedgerConfig::default()
    };
    let mut l = new_ledger_with(
        &d,
        as_dyn(&clk),
        cfg.clone(),
        Arc::new(QuotaOpener(budget.clone())),
    )
    .unwrap();
    l.append_all(scenario()).unwrap();
    let digest_before = l.state().digest();
    let m_before = l.manifest().clone();
    // the new segment's first record (segment_opened) fails: the manifest was already switched (commit point),
    // the handle is poisoned, and the reopened ledger is valid and equals the state before.
    budget.store(10, Ordering::SeqCst);
    let r = l.rotate();
    assert!(matches!(r, Err(LedgerError::NoSpace { .. })), "{r:?}");
    drop(l);
    budget.store(i64::MAX, Ordering::SeqCst);
    let (l2, _) = open_ledger_with(
        &d,
        as_dyn(&clk),
        cfg,
        Arc::new(QuotaOpener(budget)),
        OpenOptions::default(),
    )
    .unwrap();
    assert_eq!(l2.state().digest(), digest_before);
    assert_eq!(l2.manifest().sealed.len(), m_before.sealed.len() + 1);
    assert!(ledger::verify_chain(&d).unwrap().snapshot_matches == Some(true));
}
