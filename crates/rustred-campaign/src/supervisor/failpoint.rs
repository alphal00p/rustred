//! Failpoints for interrupt tests and rehearsals (plan 6.5 I1; interfaces/supervisor.md section 14):
//! `RR_CAMPAIGN_FAILPOINT=<site>:<n>` aborts the supervisor itself (SIGABRT, no unwinding) at the n-th pass of `<site>` (1-based). The
//! special site `ledger_enospc` makes the n-th ledger append and every later one fail with ENOSPC.
//!
//! TEST BUILDS ONLY (procfix3, gate-A M-2): the variable is read and the sites exist only with `debug_assertions` (the
//! profile `cargo test` and the test binaries' `rr_campaign` use). A release build (the user's run) compiles all of it out:
//! it never reads `RR_CAMPAIGN_FAILPOINT`, has no abort site and no ENOSPC seam (checked with `strings` on the release
//! binary, see notes/implementer.procfix3.md). REQUEST TO I-INT: a dedicated cargo feature `failpoints` (off by default)
//! would let a release-mode rehearsal opt in explicitly; until then debug builds are the only ones with failpoints.

use crate::ledger::FileOps;
use std::sync::Arc;

/// The variable (test builds only read it).
pub const FAILPOINT_ENV: &str = "RR_CAMPAIGN_FAILPOINT";

/// BLAKE3 of the variable's NAME (gate (d), lane int5): a release build recognises the variable, and REFUSES to run
/// with it, without carrying its name (the `strings` check finds neither the name nor a site).
#[cfg_attr(debug_assertions, allow(dead_code))]
const FAILPOINT_ENV_BLAKE3: &str =
    "faf147e7d740c975e49c408a3e030816c176ef73bb706840b7b077de3b3a4d70";

/// The name of the test-hook variable when it is set in a RELEASE build (which has no hooks: running on would let an
/// operator believe a rehearsal hook is armed; the supervisor refuses, exit 1). Always `None` in a test build.
pub fn release_refusal() -> Option<String> {
    #[cfg(not(debug_assertions))]
    {
        std::env::vars_os()
            .map(|(k, _)| k)
            .find(|k| blake3::hash(k.as_encoded_bytes()).to_hex().as_str() == FAILPOINT_ENV_BLAKE3)
            .map(|k| k.to_string_lossy().into_owned())
    }
    #[cfg(debug_assertions)]
    {
        None
    }
}

#[derive(Debug, Default)]
pub struct Failpoints {
    #[cfg(debug_assertions)]
    site: Option<String>,
    #[cfg(debug_assertions)]
    n: u64,
    #[cfg(debug_assertions)]
    passes: std::sync::atomic::AtomicU64,
}

impl Failpoints {
    /// From `RR_CAMPAIGN_FAILPOINT` in a test build; always inactive in a release build.
    pub fn from_env() -> Failpoints {
        #[cfg(debug_assertions)]
        {
            match std::env::var(FAILPOINT_ENV) {
                Ok(v) => Self::parse(&v),
                Err(_) => Failpoints::default(),
            }
        }
        #[cfg(not(debug_assertions))]
        {
            Failpoints::default()
        }
    }

    /// The configured site, if any (for the supervisor's log line).
    pub fn active(&self) -> Option<String> {
        #[cfg(debug_assertions)]
        {
            self.site.as_ref().map(|s| format!("{s}:{}", self.n))
        }
        #[cfg(not(debug_assertions))]
        {
            None
        }
    }

    /// Is `site` the configured site (any pass)? A non-aborting failpoint for decisions (e.g. `final_sweep_unproven`:
    /// the final sweep reports itself unproven). Always false in a release build.
    #[cfg(debug_assertions)]
    pub fn armed(&self, site: &str) -> bool {
        self.site.as_deref() == Some(site)
    }

    #[cfg(not(debug_assertions))]
    #[inline(always)]
    pub fn armed(&self, _site: &str) -> bool {
        false
    }

    #[cfg(debug_assertions)]
    pub fn parse(v: &str) -> Failpoints {
        match v.rsplit_once(':') {
            Some((site, n)) => Failpoints {
                site: Some(site.to_string()),
                n: n.parse().unwrap_or(1).max(1),
                passes: std::sync::atomic::AtomicU64::new(0),
            },
            None => Failpoints::default(),
        }
    }

    /// Pass `site`: abort ourselves at the configured pass (test builds); nothing in a release build.
    #[cfg(debug_assertions)]
    pub fn hit(&self, site: &str) {
        use std::sync::atomic::Ordering;
        if self.site.as_deref() != Some(site) {
            return;
        }
        let k = self.passes.fetch_add(1, Ordering::SeqCst) + 1;
        if k == self.n {
            // abort(): no unwinding, no Drop, no flush (as a hard kill); the supervisor has no signal path of its own.
            use std::io::Write;
            let _ = writeln!(std::io::stderr(), "failpoint {site}:{k}: abort");
            std::process::abort();
        }
    }

    #[cfg(not(debug_assertions))]
    #[inline(always)]
    pub fn hit(&self, _site: &str) {}

    /// The ENOSPC file operations when `ledger_enospc:<n>` is configured (test builds; only segment appends fail).
    pub fn ledger_ops(&self) -> Option<Arc<dyn FileOps>> {
        #[cfg(debug_assertions)]
        {
            (self.site.as_deref() == Some("ledger_enospc")).then(|| {
                Arc::new(enospc::EnospcOps {
                    after: self.n,
                    count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                }) as Arc<dyn FileOps>
            })
        }
        #[cfg(not(debug_assertions))]
        {
            None
        }
    }
}

#[cfg(debug_assertions)]
mod enospc {
    use crate::ledger::{FileOps, RealFile, SegmentFile};
    use std::io;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Real ledger file operations whose `after`-th segment append (1-based, counted over all segments) and every later
    /// one fail with ENOSPC.
    pub(super) struct EnospcOps {
        pub(super) after: u64,
        pub(super) count: Arc<AtomicU64>,
    }

    struct EnospcFile {
        inner: Box<dyn SegmentFile>,
        after: u64,
        count: Arc<AtomicU64>,
    }

    impl FileOps for EnospcOps {
        fn open_segment(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
            Ok(Box::new(EnospcFile {
                inner: RealFile::open(path)?,
                after: self.after,
                count: Arc::clone(&self.count),
            }))
        }
    }

    impl SegmentFile for EnospcFile {
        fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
            let k = self.count.fetch_add(1, Ordering::SeqCst) + 1;
            if k >= self.after {
                return Err(io::Error::from_raw_os_error(libc::ENOSPC));
            }
            self.inner.append(bytes)
        }
        fn sync(&mut self) -> io::Result<()> {
            self.inner.sync()
        }
        fn truncate(&mut self, len: u64) -> io::Result<()> {
            self.inner.truncate(len)
        }
        fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
            self.inner.write_at(offset, bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    /// The release build's refusal matches exactly the variable's name.
    #[test]
    fn release_refusal_digest_is_the_variable_name() {
        assert_eq!(
            blake3::hash(super::FAILPOINT_ENV.as_bytes())
                .to_hex()
                .as_str(),
            super::FAILPOINT_ENV_BLAKE3
        );
        assert_eq!(super::release_refusal(), None, "a test build never refuses");
    }
}
