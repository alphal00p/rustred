//! Failpoints for interrupt tests and rehearsals (plan 6.5 I1; interfaces/supervisor.md section 14):
//! `RR_CAMPAIGN_FAILPOINT=<site>:<n>` aborts the supervisor itself (SIGABRT, no unwinding) at the n-th pass of `<site>` (1-based). The
//! special site `ledger_enospc` makes the n-th ledger append and every later one fail with ENOSPC.

use crate::ledger::{SegmentFile, SegmentOpener};
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub const FAILPOINT_ENV: &str = "RR_CAMPAIGN_FAILPOINT";

#[derive(Debug, Default)]
pub struct Failpoints {
    site: Option<String>,
    n: u64,
    passes: AtomicU64,
}

impl Failpoints {
    pub fn from_env() -> Failpoints {
        match std::env::var(FAILPOINT_ENV) {
            Ok(v) => Self::parse(&v),
            Err(_) => Failpoints::default(),
        }
    }

    pub fn parse(v: &str) -> Failpoints {
        match v.rsplit_once(':') {
            Some((site, n)) => Failpoints {
                site: Some(site.to_string()),
                n: n.parse().unwrap_or(1).max(1),
                passes: AtomicU64::new(0),
            },
            None => Failpoints::default(),
        }
    }

    /// Pass `site`: SIGKILL ourselves at the configured pass.
    pub fn hit(&self, site: &str) {
        if self.site.as_deref() != Some(site) {
            return;
        }
        let k = self.passes.fetch_add(1, Ordering::SeqCst) + 1;
        if k == self.n {
            // abort(): no unwinding, no Drop, no flush (as a hard kill); the supervisor has no signal path of its own.
            eprintln!("failpoint {site}:{k}: abort");
            std::process::abort();
        }
    }

    /// The ENOSPC segment opener when `ledger_enospc:<n>` is configured.
    pub fn ledger_opener(&self) -> Option<Arc<dyn SegmentOpener>> {
        (self.site.as_deref() == Some("ledger_enospc")).then(|| {
            Arc::new(EnospcOpener {
                after: self.n,
                count: Arc::new(AtomicU64::new(0)),
            }) as Arc<dyn SegmentOpener>
        })
    }
}

/// Real segment files whose `after`-th append (1-based, counted over all segments) and every later one fail with ENOSPC.
struct EnospcOpener {
    after: u64,
    count: Arc<AtomicU64>,
}

struct EnospcFile {
    file: std::fs::File,
    after: u64,
    count: Arc<AtomicU64>,
}

impl SegmentOpener for EnospcOpener {
    fn open(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
        let file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)?;
        Ok(Box::new(EnospcFile {
            file,
            after: self.after,
            count: Arc::clone(&self.count),
        }))
    }
}

impl SegmentFile for EnospcFile {
    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        use std::io::Write;
        let k = self.count.fetch_add(1, Ordering::SeqCst) + 1;
        if k >= self.after {
            return Err(io::Error::from_raw_os_error(libc::ENOSPC));
        }
        self.file.write_all(bytes)
    }
    fn sync(&mut self) -> io::Result<()> {
        self.file.sync_data()
    }
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        self.file.set_len(len)?;
        self.file.sync_all()
    }
}
